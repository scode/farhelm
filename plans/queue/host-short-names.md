# Hosts: a short name for every host, shown on the sidebar rail

Written against main at c2789e25 on 2026-10-29. This is a goal file for an unattended run, executed through the planning
system in `plans/AGENTS.md`. You, the executing agent, have none of the conversation that produced it; everything you
need is here or in the repository files it names. SPEC.md and SPEC_impl.md stay authoritative over anything this file
says, except where this file records a change the maintainer approved; that change is yours to make in the spec.

## The goal

Two TODO.md entries (bucket "Near term"), verbatim, planned together at the maintainer's choice:

> - **Auto-assign a host alias from the left-most hostname label.** When a host is added, derive its alias automatically
>   from the left-most part of the hostname (for example, `my-server.example.com` becomes `my-server`) instead of
>   leaving it unset or requiring a manual pick. Plan: `plans/queue/host-short-names.md`.

> - **Make host aliases/names visible in the new layout.** Consider how the new host list can show each host's alias or
>   name at a glance; today they are easy to miss. Plan: `plans/queue/host-short-names.md`.

The maintainer overrode the first entry while planning: no alias is ever set automatically. Instead, every host has a
**short name** derived by one rule, and the rail shows it. In their words: "Let's just make that the rule that we put in
the spec, that when we show a host anywhere and the situation is such that we want the shorter form, we always extract
the shorter form using the same rule, which is the first component of the hostname, unless there is an explicit alias
given, in which case we use the alias. I think this is cleaner than introducing the concept of actually auto-setting the
alias. This keeps the user's intent intact, so if the user does have an alias, it's there. If they don't, we know that
they don't, but we're still able to use the shorter version whenever we want in a consistent way. This overrides the
TODO item."

On the rail itself: "My initial gut instinct when I see the lists to the left in the new UI is: what are these icons? I
don't intuitively get what those things are. If we can include the alias of the host somehow ... it would also act as a
legend."

### The short name

A host's short name is a function of that host alone (its kind, destination and alias), never of the other hosts:

- If the host has an alias, the alias, exactly as stored.
- Otherwise, for the local host: `Local`.
- Otherwise, from the SSH destination: strip an `ssh://` scheme; strip a user, splitting on the last `@` as OpenSSH
  does; strip a port only where one can appear (the `ssh://` form's `:port`, or after a bracketed IPv6 literal); remove
  brackets. If what remains is an IP address, the short name is the whole address (never cut at a dot or colon). If it
  is not, take it up to its first dot. A dot-less name (an ssh config alias such as `gpubox`, a zone-scoped IPv6 such as
  `fe80::1%en0`) is used as it is. If the first label would be empty (a leading dot), use the whole remainder.

Two hosts can have the same short name; that is accepted. The short name is display only: it never identifies or
resolves a host (agent requests keep resolving by display name).

The maintainer asked for this to be a first-class concept, not local UI code: "we should have a single implementation of
"host -> short display name" that is a function only of the host itself rather than a function of everything in a larger
list. we can consider adding this complexity later in response to feedback if needed. Make "short display name" a first
class concept in the code (SPEC_impl) so that agents will use it. Don't just make it some local code in the UI stack,
make it a proper thing you can ask a host for in the code."

### What changes on screen

- **Rail, collapsed:** each host avatar has its short name as a small caption underneath, truncated with an ellipsis to
  the rail's width. The rail stays 56px wide. The local host's caption is its short name (`Local`, or its alias), not
  today's hard-coded "This machine".
- **Rail, expanded:** while the pointer rests on the rail for a moment, and while keyboard focus is inside it, the rail
  widens over the session list (overlaying it, not pushing it) and shows each name to the right of its icon: the host's
  display name, with the full destination underneath in smaller text when it differs. The local host's display name is
  the one the Hosts panel uses ("Local (this machine)", or its alias). The All and Add host buttons show their labels
  too. Icons stay where they are as the rail widens. The Hosts, Settings and Help buttons at the rail's foot stay 56px.
  Clicking, menus and badges are unchanged.
- **Session rows:** the host line shows the host's short name when the session's host is in the hosts read; otherwise it
  keeps the full name the helm supplies, as today.
- Everything else (the Hosts panel, host details, dialogs, tooltips, agent output) keeps the display name.

### What does not change

Aliases themselves: what the user sets, its validation and uniqueness, and the Add host dialog. The helm's
`host_display_name` and every name the helm sends. Agent host resolution. Host icons and colors. The rail's order,
filtering behavior, badges, menus and scrolling.

### Acceptance criteria

- One function computes the short name, in `farhelm-proto`, over raw text, with unit tests for: alias set; local with
  and without alias; plain host; FQDN; `user@host`; a user containing `@`; `ssh://user@host:2222`; `ssh://[::1]:22`;
  IPv4; bare IPv6 (`fe80::1`, `user@::1`); zone-scoped IPv6; dot-less ssh config alias; leading dot.
- The UI's `Host` type has a `short_name()` method that delegates to it, and every UI surface that wants the short form
  calls that method. SPEC_impl.md names both as the only way to get a host's short form.
- The rail shows the captions and the expanded overlay as described, in Chromium and WebKit, without moving the icons
  and without a screen reader announcing each name twice.
- Session rows show the short name for a known host and the helm's full name otherwise.
- SPEC.md states the short-name rule beside the alias paragraph and updates the rail paragraph (including the local
  host's wording) and the session row host-line sentence; SPEC_impl.md's local-host labelling paragraph is revised to
  match.
- A changelog fragment (`kind: changed`, or `added` if the captions read better as new).
- Both TODO.md entries quoted above are removed in the plan's last PR.

## Decisions already made

Settled with the maintainer on 2026-10-29 on a planning page with mockups:

- **One plan for both entries.**
- **No automatic alias;** the short-name rule instead, in the maintainer's words above.
- **Rail design:** "let's do (A) combined with (D). shortname is used if available, else fullname. truncated as needed.
  when rail expands you see the full name in the proposed layout. so when not expanded, name "under" icon; when
  expanded, name to the "right" of the icon". Not chosen: a wider permanent rail, and letters inside the avatar.
- **Expanded content:** the display name with the full destination underneath when it differs.
- **Short-name rules** as listed above (the planner's proposal, accepted; the `ssh://`, IPv6 and leading-dot details
  were added by the planning review and follow from it).
- **Duplicates allowed,** for simplicity; disambiguation can come later on feedback.
- **Where it appears:** the rail and session rows' host line; everything else keeps the display name.
- **Code reviewer:** a single fresh-context gpt-6.1-sol at high effort. No substitutes.
- **No-workhorse mode**, the plans default, stated on the planning page.

## Requirement sources

**The user's request:** the two TODO.md entries, the maintainer's words quoted above, and the decisions.

**Binding repository constraints:** SPEC.md's host registry paragraph on aliases ("A host can carry an optional alias"),
its Session list paragraphs on the host rail ("This machine names the registered local host") and on the session row's
host line ("shows the helm-supplied host name"); SPEC_impl.md's local-host labelling paragraph ("confirmed-local session
rows"); the `Host` mirror's rule that registry text keeps its raw wire spelling until presentation
(`crates/farhelm-ui/src/lib.rs`); root `AGENTS.md`, "Code rules", "Tests and validation", "Conventional Commits",
"Changelog fragments"; `.agents/test-authoring.md`; `.agents/validation.md`.

**Planner proposals** (change them if the code says otherwise, logged as a DECISION): the function's signature
(`host_short_name(local: bool, destination: Option<&str>, alias: Option<&str>) -> String`, since the UI's host kind has
an `Unrecognized` variant the helm's lacks) and module; computing the short name in the UI from fields it already
receives rather than adding wire fields; the expanded rail in CSS (`:hover` with a transition delay for the dwell,
`:focus-within`) rather than Rust timers; captions kept out of the button's accessible name; two PRs. A fresh-context
planning review checked the outline against the code and contributed the no-wire-change design, the destination forms,
the local-host wording gap, the raw-text and double-announcement notes, and the CSS expansion.

## Outline

### PR 1: the short name as a concept

1. A small module in `crates/farhelm-proto` (beside `host_appearance.rs`) with the pure function and its tests. It works
   on raw peer text; callers escape the result when they render it, as they do for every other host string. `core::net`
   address parsing works on wasm.
2. `Host::short_name(&self)` on the UI mirror in `crates/farhelm-ui/src/lib.rs`, using `kind.is_this_machine()` and the
   raw destination and alias.
3. SPEC.md: the rule next to the alias paragraph. SPEC_impl.md: where the function lives, that it is the only way to get
   the short form, that it is display only, and that the helm sends nothing new for it (every input already reaches the
   UI on `GET /api/hosts`). No helm change and no HTTP contract fixture change.

### PR 2: the rail and session rows

1. **Captions.** In `HostRailAvatar` (`crates/farhelm-ui/src/hosts.rs`), render the short name under the icon, outside
   the labelled button or `aria-hidden`, since the button's accessible name already carries the name. Replace the
   hard-coded "This machine" in the avatar's tooltip and label with the Hosts panel's display name for the local host.
   CSS in `crates/farhelm-ui/assets/app.css`: the caption's size and truncation, and the rail's vertical spacing so the
   avatars and captions fit (the rail already scrolls independently).
2. **Expansion.** The rail is already absolutely positioned beside the sidebar content, so widening it overlays the list
   with no layout change. Expand with `.host-rail:hover` after a short transition delay and with `:focus-within`;
   collapse when neither holds. Left-anchor the icons at a fixed x in both states (today `align-items: center` would
   shift them, and rail menus anchor to the avatar), or keep the rail expanded while one of its menus is open; log
   which. In the expanded state show the names to the right of the icons (display name, destination underneath when
   different), and the All and Add host labels. Leave `.host-rail-footer` at 56px.
3. **Session rows.** In `crates/farhelm-ui/src/list/view.rs`, where `RowState` already joins the hosts read for each
   session's locality, icon and color, join the host's `short_name()` too, and have the row's host line
   (`crates/farhelm-ui/src/list/row.rs`, `SessionHostName`) use it, falling back to the session's helm-supplied
   `host_name` when the host is not in the read.
4. **Spec, changelog, TODO.** SPEC.md's rail paragraph and the session row host-line sentence; SPEC_impl.md's local-host
   labelling paragraph; a changelog fragment; remove both TODO entries.
5. **Tests.** UI unit tests where the crate already tests the rail and rows. Browser tests: captions show the short name
   (aliased, unaliased FQDN, local), truncation does not overflow, the overlay opens on hover and on keyboard focus and
   shows display name plus destination, icons do not move between states, and a session row's host line shows the short
   name. Look at the existing rail and Hosts browser tests for how hosts are registered with given destinations.

Before touching these paths, read the instruction file root `AGENTS.md`, "Instructions by path", names for them:
`.agents/validation.md` and `.agents/test-authoring.md` for the tests.

### Validation

Per root `AGENTS.md`, "Tests and validation", and `.agents/validation.md`, choose the smallest set of checks that would
expose a regression, and report checks run, reused and skipped. Expect `cargo fmt`, clippy with `-D warnings` on
`farhelm-proto` and `farhelm-ui`, both crates' tests (through the pinned nextest setup and
`python3 scripts/record-test-run.py`, per `.agents/test-evidence.md`, where they are runtime tests), `dprint check` on
changed Markdown, `python3 releasing/check-changelog.py format`, and the new and affected rail, Hosts and session-row
browser tests on Chromium and WebKit through the recorder. Run `python -B scripts/check-test-sleeps.py` when Rust or
browser tests change.

## Fallbacks

Departures that need the scope reassessment: setting an alias automatically; disambiguating duplicate short names; using
the short name anywhere beyond the rail and session rows' host line; using it to resolve a host; adding helm or wire
changes for it; widening the collapsed rail; Rust-side timers or state for the expansion; changing alias validation.

Decisions to log: the function's signature and module, how the icons stay put during expansion, how the caption avoids a
double announcement, the changelog kind, and every review finding you decided not to follow.

## How to run

### Files outside the repository

Per `plans/.agents/outside-files.md`, "Files outside the repository": this plan's working log is
`farhelm-plan-host-short-names-log.md` in the parent directory of the checkout you run in, derived as that section says.
Resolve it to an absolute path before you start. Scratch files go in the agent scratch directory that `AGENTS.md`,
"Agent scratch space", describes, not in the checkout.

### Resume protocol

Your first action is to invoke the `agent-resumeable` skill with the log file's absolute path. If the log exists, read
it and resume where the previous session left off, cross-checking it against reality (the jj graph, bookmarks and open
PRs) rather than starting over. If it does not exist, this is a fresh start. A plan that an earlier executor worked on,
or that came back from review, also gets the resume check in `plans/.agents/executing.md`, "Executing one plan" (step 7)
before any work.

### Execution mode: no-workhorse

Never send a unit of your own decomposition to a weaker model or a lower effort than your own. Never invoke, load or
follow `scode-galaxy-brain` or `scode-model-routing` during this plan. No-workhorse is the mode
`plans/.agents/planning.md` sets for plans, and the maintainer saw it stated while planning, so it is an explicit
choice: if galaxy-brain is already active when you start, this file is the instruction to stop it.

Sub-agents at your own model and effort are fine, and worth considering when they are likely to finish the work sooner
without costing quality (independent investigations in parallel, say). Concurrent agents that might otherwise conflict
can work in separate jj workspaces whose results you merge back; you still own the stack and every PR. Launch a
sub-agent natively, with model and effort set explicitly where the mechanism allows; when the model you need is not
available as a native sub-agent type (the code reviewer below, for instance), `$scode-harness-shellout` supplies the
launch mechanics.

The review gate keeps the model and effort this file demands; the scope reassessment runs at your own model and effort.
The sub-agents `plans/AGENTS.md`, "Sub-agents this file requires", requires of every plan (the resume check, and the
cold reads of a blocked question and of the report) run as that file says: native, read-only, at your own model.

### Resource watchdog

Right after the resume protocol, and before the first sub-agent, build or test run, start a resource watchdog as a
background process (not an agent) and keep it running for the whole run. Every 60 seconds it samples free space on the
filesystems holding the checkout, any jj workspaces you create, the agent scratch directory and `/tmp`, plus available
memory and swap (`df`, and `free` or `/proc/meminfo` on Linux; `vm_stat` and `sysctl` on macOS). It writes each sample
to a private heartbeat/status file in the scratch directory, and it emits a notification when any of these happens:

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

On every path, check heartbeat freshness before each new build, test run, sub-agent or review launch. A dead monitor, or
a heartbeat stale for two sample intervals, pauses new launches until monitoring is restored; restart a dead watchdog.

An alert is an instruction to act:

1. stop launching sub-agents and work;
2. remove jj workspaces, build output and scratch you own that are no longer needed;
3. wait for or stop the job most likely responsible;
4. resume only when the watchdog reports headroom.

Record the watchdog's handle, watched paths, status path and delivery mechanism in the log. Include its state in every
handoff note so a resumed session reconciles or restarts it, and stop it when the plan closes. Do not lengthen the
sampling interval to save turns.

### PR discipline

- One reviewable commit, stable bookmark and draft PR per step, in a single linear stack, using the `jjstack` skill. The
  stack's base is `main@origin`, or this plan's own open PRs when it resumes, set up per `plans/.agents/executing.md`,
  "Executing one plan" (step 6); never another plan's PRs. Bookmarks are `plan/host-short-names/<nn>-<short-name>`
  (expected: `01-short-name` and `02-rail-captions`).
- Avoid churn: code added in one PR and deleted in a later one means the stack should have been shaped differently.
  Within this run, a PR that needs correcting is restructured rather than corrected on top, and that applies to PRs an
  earlier run built too.
- Commit messages and PR titles use Conventional Commits, with the type reflecting the user-visible effect (root
  `AGENTS.md`), and a changelog fragment in the same commit when the type calls for one (root `AGENTS.md`, "Changelog
  fragments").
- Run every commit message, PR title and PR description through the `scode-commit-msg-reviewer` skill's cold read.
- `TODO.md`, `TRIAGE_OUTCOMES.md`, `review_feedback_queue/` and `releasing/changelog.d/` are shared bookkeeping files:
  on a rebase conflict there, keep both sides.
- Before every push of a plan bookmark, run `queue status` and confirm the line still carries your claim id
  (`plans/.agents/executing.md`, "Executing one plan", step 9).
- Never edit `plans/`: the plan's state moves only through `scripts/plans-queue.py`, as `plans/AGENTS.md` describes.
- PRs stay drafts. Never mark one ready and never merge; landing is the monitor's job (`plans/AGENTS.md`).

### Code review gate

Before finishing each PR, launch a review of its changes and address what it finds before moving on. The maintainer
demands exactly this reviewer, and no review swarm: a single fresh-context agent on gpt-6.1-sol at high effort, launched
through `$scode-harness-shellout` when your harness cannot serve that model natively. If it cannot be reached when a
review is due and is still unreachable after a reasonable retry, block the plan per `plans/.agents/executing.md`,
"Executing one plan" (step 10), after pushing what is consistent. Never substitute another reviewer.

The prompt carries this full charter, because the reviewer has nothing else:

> Review the changes in this PR adversarially for general correctness (bugs, unhandled cases, broken invariants, wrong
> behavior against the acceptance criteria quoted below), design (whether the shape of the change fits the codebase and
> the goal, and whether a simpler shape would), and idiomatic code for the languages involved. Report findings to the
> named file, most severe first, each with the file, the quoted code, what is wrong, and the smallest fix. Edit nothing
> and touch no VCS state.

The prompt must also name the repository root (the executing checkout), the bookmark or commit range of this one PR, a
findings file in the scratch directory, and the acceptance criteria (this file's goal, the short-name rule, what changes
and what does not, acceptance criteria, decisions and outline). When a PR changes tests or their fixtures, include the
full text of `.agents/test-authoring.md` verbatim in the prompt, as root `AGENTS.md` requires.

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

Reuse an existing review checkpoint when it can answer the question; do not schedule periodic reviews. A sub-agent gets
a bounded unit within the outline; an unapproved new mechanism is outside its task, and one discovered mid-unit is left
unimplemented and reported back, with checks and incomplete work stated honestly. Apply the same decision boundary as
Unattended fallback below.

### Decision logging

Log each consequential choice in the working log as it happens, under a scannable `DECISION` label, with the
alternatives considered: in particular those named under Fallbacks above, and every review finding you decided not to
follow. The user will ask for these later.

### Unattended fallback

Resolve routine implementation forks within the agreed scope and log them as DECISIONs. Remove planner-invented
machinery that turns out to be unnecessary, as long as the user's decisions and the repository constraints still hold.
Wording in this file that turns out inaccurate (a line number, a file name, a class name) is corrected to the truth.

A material scope expansion, a weakened guarantee, or an omitted required behavior needs an agreed fallback or the user's
decision; a review finding or a log entry is not authorization. If no fallback covers it, record the concrete tradeoff
and continue with independent work. Block per `plans/.agents/executing.md`, "Executing one plan" (step 10) only for what
cannot proceed without the user, after pushing every PR that can be finished.

You run unattended, so never ask the maintainer anything in conversation: no question tool, no approval prompt, and no
"should I ...?" or offer of optional extras closing a turn, whatever a loaded skill or the harness suggests. Every fork
is either decided by you, taking the option that best fits this file, its Decisions and the specs, and logged as a
DECISION, or it is a block per `plans/AGENTS.md`. Things the maintainer should know and possible follow-ups go into the
report's "things you should know" and "open questions and possible follow-ups" sections, never into a question.

## Done criterion

The plan is complete when a linear stack of draft PRs exists that together meets the acceptance criteria above, each PR
passed its code review gate, and the last PR removes both TODO.md entries. Open, not merged. If a `## Decisions` section
exists, its latest entry must also be satisfied. Then close the plan per `plans/.agents/executing.md`, "Executing one
plan" (steps 11 and 12): deliver its report through the queue script, write a closing entry in its log, and stop the
watchdog. Never edit `plans/` yourself.

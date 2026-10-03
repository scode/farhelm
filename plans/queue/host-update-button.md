# An update button on hosts that run an older Farhelm

Written against main at ea5bf905 on 2026-10-02. This is a goal file for an unattended run, executed through the planning
system in `plans/AGENTS.md`. You, the executing agent, have none of the conversation that produced it; everything you
need is here or in the repository files it names. SPEC.md and SPEC_impl.md stay authoritative over anything this file
says, except where this file names a passage the user decided to change.

This plan runs after `plans/queue/host-dialogs-and-menu.md`, which reworks the same host row and its menu
(hover-revealed toggle, session-style menu, removal in a dialog). The dependency is about avoiding churn in shared code,
not about building on anything from it: read what it landed and fit your change to the row as it is then. If it has not
landed and the inline removal prompt still exists, disable the new button while that prompt shows, as the `⋯` toggle is.

## The goal

When a remote host runs an older Farhelm than the helm, its row in the host list shows a clickable update button in
place of today's `old version` or `needs update` word. A required update (the helm refuses to talk to the host) looks
distinct from an optional one, hovering says what clicking does and whether it is required, and clicking starts the same
update the host menu's Update starts. The menu keeps its Update item unchanged.

Acceptance criteria:

- The button shows on an ssh host whose row reads `old version` (connected, compatible, older build: update optional) or
  `needs update` from a protocol skew where the host's protocol is the lower one (update required), whenever Update is
  currently available for that host. It never shows on a host on the current version, a too-new host, or the local host.
- Optional: an outlined amber button (the same amber as today's `old version` word); required: an outlined red button
  (the same red as today's `needs update` word); both with a small up-arrow before the word "update".
- Hover names the host's version and the helm's (and both protocol versions for a skew) and says whether the update is
  required or optional and that clicking updates the host to the helm's version.
- Clicking sends exactly the request the menu's Update sends, with no confirmation. While the update runs, the existing
  inline progress takes the button's place; the row never shows both.
- Wherever the button is not offered (an add or another run in progress on the host, the row's update options not yet
  loaded, an old-version local host), the plain word shows as today.
- Screen readers: the host status keeps its words for an outdated host, and the button has its own accessible name.
- SPEC.md, SPEC_impl.md and `website/src/content/docs/docs/using/manage-hosts.md` describe the button. The TODO.md entry
  is removed.

## Requirement sources

**The user's request:** "then also plan "update button on hosts"", for the TODO.md `Near term` entry, verbatim as of
ea5bf905: "**An update button on hosts that can be updated.** When a host in the host list can be updated (an old
version, or one that needs an update), show an actual clickable "update" button there instead of only text. Keep the
update option in the host's pop-up menu as well. When an update is not just possible but required, the button should
look different, probably red; the exact treatment is for design time. Hovering over the button should say what clicking
it does and whether the update is required or merely possible; wording TBD."

**The user's decisions (2026-10-02):**

- U1. Which hosts: only ssh hosts older than the helm, `old version` (optional) and `needs update` (required). No button
  on current, too-new or local hosts. The menu's Update stays exactly as it is ("yes correct").
- U2. Look: the user said "the needs update needs to be visually distinct, probably red, signalling its required. that
  said, its a different meaning than what we otherwise use red for so i'm open to suggestions", then agreed to: an
  outlined button colored like the row's status word, amber for optional and red for required, with a small up-arrow
  before "update". Outline rather than fill so it does not read as a destructive button; the color says how urgent the
  update is, as the status word already does.
- U3. Placement: the button replaces the `old version` / `needs update` word; the status dot keeps its color. Hover as
  in the acceptance criteria. Click does what the menu's Update does, without confirmation; progress replaces the
  button.
- U4. Review gate: a fresh-context Opus 5.5 reviewer at high effort, told to review adversarially. No review swarm.
- U5. No-workhorse mode: you do all the work yourself (see How to run).

**Binding repository constraints:**

- SPEC.md Session list: the row "says `old version` as an advisory; an incompatible protocol handshake remains
  `needs
  update`" and "Host actions open on demand from the row menu"; SPEC.md Errors and diagnostics: "Actions stay
  in each row's menu". Amend all three for this one inline action.
- SPEC_impl.md, GUI: Dioxus: the button tiers paragraph ("One deliberate fourth look exists: ..."; "the sidebar's
  resting chrome carries exactly one filled control (`new session`)"). Restructure the paragraph so the status-toned
  outlined update button is a named deliberate look (it is outlined, so the one-filled-control rule still holds). The
  host-list paragraph describing `old version` and `needs update` changes too.
- `.host-row-main` is `nowrap` and `.host-name` has a 4em floor (app.css comments explain the width budget); the button
  must fit the same line without clipping the name below its floor or pushing the `⋯` toggle out.
- The browser tests find the menu item by `.provisioning-update`, often with `toHaveCount(0)`; the button must not carry
  that class. The e2e helm is a development build, so real hosts never read old version; tests stub `/api/hosts`.
- Root `AGENTS.md`: a `feat` PR carries a changelog fragment and removes the TODO entry it addresses.

**Planner choices (from the planning review):**

- P1. Reuse the "update all" eligibility. `available_remote_updates` in `crates/farhelm-ui/src/hosts.rs` already decides
  which remote hosts can be updated right now (`updates_automatically()`, not too new, not busy, the menu offering
  Update). Factor its per-host test into a small helper both use; the button shows when that helper passes and the phase
  is `Connected { old_version: true }` or a `VersionSkew` with `peer_protocol < our_protocol`. If threading the busy set
  into `HostRow` is awkward, the menu's own `update` flag plus `updates_automatically()` is enough, since U3 only asks
  for parity with the menu. Use `updates_automatically()`, not a literal ssh comparison.
- P2. One handle for required vs optional (a modifier class or a data attribute, not both), used by CSS and tests. The
  red keys off that handle, not off the status's `needs-attention` class, which identity problems share. The button sits
  outside the `role="status"` span and needs its own amber (`--warn`) and red (`--danger-strong`) rules; precedent for
  an outlined red button: `.yolo-confirm-stop-asking`.
- P3. Hover text is one sibling of `too_new_title`, built the same way (same peer-text escaping, same version sources).

## Implementation outline

One PR (`feat:`). Line numbers drift; find the code by name.

- `crates/farhelm-ui/src/hosts.rs`: the eligibility helper (P1), the hover-text helper (P3), and in `HostRow`'s
  `.host-row-main` a `button.btn.host-update-button` (pick the final class name) between the status and the `⋯` toggle,
  rendered only when eligible and no update progress is showing. Its click sends the same `ActionRequest{Update}` the
  menu item sends through `on_provisioning`. When the button shows, the visible label word is not rendered, but the
  status keeps the word as visually hidden text or an `aria-label`.
- `crates/farhelm-ui/assets/app.css`: the outlined amber and red treatments and the arrow, sized like the hosts header's
  small buttons.
- Tests:
  - Rust unit tests next to the `phase_display_label` / `too_new_title` tests: eligibility across phases and kinds, and
    the hover text for both cases.
  - Browser: add an old-version connected row and a lower-protocol skew row to the stubbed `host-list-phase-table` test
    in `e2e/tests/terminal-multihost.spec.ts`, asserting the button, its required/optional handle and its hover, and
    asserting no button on the other rows (cover the local host too: that test's stub filters the local row, so keep it
    or add a stubbed local old-version row). Adjust that loop's `.host-status-label` assertions for the new rows. One
    click test: stub a row, route and hold the update plan request, click, assert the request went out for that host and
    `updating…` progress replaced the button; fold the one-line layout check (as in
    `host-menu-survives-the-longest-phase-word`) into it. No real update: the menu's tests in
    `e2e/tests/provisioning.spec.ts` already cover that path end to end.
- SPEC.md, SPEC_impl.md and `manage-hosts.md` as in the constraints. Changelog fragment `kind: added`. Remove the TODO
  entry.

### What not to build

No new state, endpoint or request path; no confirmation step; no button on current, too-new or local hosts; no change to
the menu's Update or to "update all" behavior beyond sharing the eligibility helper.

## How to run

### Files outside the repository

Per `plans/AGENTS.md` (Files outside the repository): this plan's working log is
`farhelm-plan-host-update-button-log.md` in the parent directory of the checkout you run in, derived as that section
says. Resolve it to an absolute path before you start. Scratch files go in the agent scratch directory root `AGENTS.md`
describes, not in the checkout.

### Resume protocol

Your first action is to invoke the `agent-resumeable` skill with the log file's absolute path. If the log exists, read
it and resume where the previous session left off, cross-checking it against reality (the jj graph, bookmarks, open PRs,
TODO.md and `plans/queue/INDEX.md`) rather than starting over. If it does not exist, this is a fresh start. A plan that
an earlier executor worked on, or that came back from review, also gets the resume check in `plans/AGENTS.md` (Executing
one plan, step 7) before any work.

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
  `plan/host-update-button/<nn>-<short-name>`.
- One commit, bookmark and draft PR. Within this run, if it needs correcting, restructure it rather than stacking a
  correction on top.
- Commit message and PR title use Conventional Commits; the PR is `feat:`. It adds its changelog fragment under
  `releasing/changelog.d/` in the same commit, per root `AGENTS.md` (Releases and the changelog); validate with
  `python3 releasing/check-changelog.py format`.
- Run the commit message, PR title and PR description through the `scode-commit-msg-reviewer` skill's cold read. Leave
  the PR description empty when the diff and title say everything.
- The PR stays a draft. Never mark it ready and never merge; landing waits until the maintainer has reviewed this plan's
  report (`plans/AGENTS.md`).

### Validation

Follow root `AGENTS.md` "Finishing work": the smallest set of checks likely to expose a regression, through
`scripts/record-test-run.py` for test execution. Typical choices, not a checklist: `cargo fmt --all -- --check`,
`cargo
clippy --all-targets -- -D warnings`, `cargo check -p farhelm-ui --features desktop`, the farhelm-ui unit tests
for `hosts.rs`, the changed Playwright tests plus `provisioning.spec.ts`'s update tests that share the request path, on
Chromium and WebKit through the recorder, `dprint check` on changed Markdown,
`python3 releasing/check-changelog.py
format`, the website build
(`cd website && bun install --frozen-lockfile && bun run build`) because a docs page changes, and
`python -B scripts/check-test-sleeps.py` per `docs/test-sleep-check.md` with `.agents/test-authoring.md` applied, since
tests change.

### Review gate

Before finishing the PR, use the active galaxy-brain skill to delegate one review of its changes. The user demands a
fresh-context agent on Opus 5.5 at high effort, reviewing adversarially (U4), shelled out to the other harness if the
executing one cannot reach that model natively. No review swarm. The prompt carries the full charter, because the
reviewer has nothing else:

> Review the changes in this PR adversarially: assume there are defects and hunt for them. Cover general correctness
> (bugs, unhandled cases, broken invariants, wrong behavior against the acceptance criteria quoted below), design
> (whether the shape of the change fits the codebase and the goal, and whether a simpler shape would), and idiomatic
> code for the languages involved. Report findings to the named file, most severe first, each with the file, the quoted
> code, what is wrong, and the smallest fix. Edit nothing and touch no VCS state.

The prompt must also name the repository root (the executing checkout), the bookmark or commit range, the findings file
(in the scratch directory), and the acceptance criteria: The goal, U1-U5 and P1-P3. Include the full text of
`.agents/test-authoring.md` verbatim, as root `AGENTS.md` requires for test changes. Address what the reviewer finds
before moving on, and log the DECISION where you decline a finding. Do not write a launch command here or in the log;
the harness-shellout skill owns launch mechanics.

### Scope reassessment

Before implementing a substantial departure from the outline above (new persistent state, a new endpoint or request
path, a confirmation step, a second eligibility rule beside the "update all" one; these are examples, not a blacklist),
and whenever the same component has needed repeated corrective review rounds, run a fresh-context review through
galaxy-brain with this charter, supplying the request, the user decisions above, this outline, the current diff and the
proposed departure (what changed, why it is necessary, and which simpler alternative was ruled out):

> Identify where this plan does more than the user needs. For each substantial new mechanism, explain which requirement
> makes it necessary and whether an existing facility or narrower supported behavior would suffice. Propose the smallest
> coherent design. Challenge planner-authored requirements; identify any simplification that would change an explicit
> user requirement or repository constraint. Report concrete findings and alternatives, or say no unnecessary complexity
> was found. Do not manufacture objections or add safeguards to justify an unnecessary subsystem. Write findings to the
> supplied private file; change no product files or VCS state.

Reuse an existing review checkpoint when it can answer the question; do not schedule periodic reviews.

### Decision logging

Log each consequential choice in the working log as it happens, under a scannable `DECISION` label, with the
alternatives considered: in particular the eligibility helper's shape, the required/optional handle, the hover wording,
the button's accessible name, the arrow (a text glyph or an icon), how the width budget was kept, and every reviewer
finding you declined. The user will ask for these later.

### Unattended fallback

Resolve routine implementation forks within the agreed scope and log them. Remove planner-invented machinery that turns
out unnecessary when the decisions still hold. A material scope expansion, a weakened guarantee, or an omitted required
behavior needs an agreed fallback or the user's decision; a review finding or a log entry is not authorization. Anything
that needs a decision: record the concrete tradeoff and block per `plans/AGENTS.md` (Executing one plan, step 10).

## Done criterion

The plan is complete when its one draft PR exists, satisfies the acceptance criteria, has passed the review gate, and
has removed the TODO.md entry. Open, not merged: merging happens only after the maintainer has reviewed this plan's
report. If a `## Decisions` section exists, its latest entry must also be satisfied. Then close the plan per
`plans/AGENTS.md` (Executing one plan, steps 11 and 12): deliver its report through the queue script, write a closing
entry in its log, and stop the watchdog. Never edit `plans/` yourself.

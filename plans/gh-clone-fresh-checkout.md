# Clone or Replace-with into a gh: checkout gets a fresh name instead of a conflict

Written against main at ea5bf905 on 2026-10-02. This is a goal file for an unattended run, executed through the planning
system in `plans/AGENTS.md`. You, the executing agent, have none of the conversation that produced it; everything you
need is here or in the repository files it names. SPEC.md and SPEC_impl.md stay authoritative over anything this file
says, except where this file names a passage the user decided to change.

This plan has no dependency on another plan.

## The goal

Clone and Replace with open the new-session form pre-filled from the source session, title included. When the user then
picks a fresh GitHub checkout (`gh:`) as the destination, Farhelm treats the copied title as a name the user typed,
names the checkout directory after it, finds that directory taken (it is usually the source's own checkout), and
refuses: "a directory with that checkout name already exists; choose another title". The fix: once a fresh checkout is
the destination, a copied title the user has not edited is not a typed name. The session is treated as unnamed, so its
title and directory become the next free `repo-N`, and the name field shows that instead of the ignored copy.

Acceptance criteria:

- Clone of a session, then choosing a fresh checkout of any repository, launches without a conflict: the new session's
  title and checkout directory are the next free `repo-N`. The same for Replace with.
- While the destination is a fresh checkout and the copied title is unedited, the name field is empty and shows the name
  the session will get (the preview's `repo-N`) as placeholder text. It never shows the copied title.
- A title the user types is used as typed, as today; if that name is taken, the existing refusal stands.
- Switching the destination back to an ordinary folder restores ordinary Clone behavior with the copied title.
- Clone with no `gh:` choice still shares the source's working copy (unchanged; SPEC.md requires it).
- The three TODO.md entries listed under Requirement sources are removed.

## Requirement sources

**The user's request:** "plan "clone of a gh" and "clone then gh" together. i think an agent split them up incorrectly
but let's talk about it during planning what the two are". The TODO.md `Near term` entries, verbatim as of ea5bf905:

- "**Clone of a gh: checkout session reuses the same working copy.** Cloning a session launched on a `gh:` fresh
  checkout, then renaming the clone, still left the clone on the original session's working copy rather than a checkout
  of its own. The maintainer knows the cause and can fix it up by hand, but the clone flow for these sessions needs
  improving. Details TBD."
- "**Clone then gh: should pick a fresh session name and checkout directory.** When the user clones a session and enters
  `gh:some/repo` as the clone's target, Farhelm should allocate a new session name and checkout directory for it. Today
  the default experience is an error saying the checkout conflicts with the existing one. Closely related to the entry
  above on clones reusing the original working copy. Details TBD."
- "**"Replace with" a gh: checkout refused because the checkout path exists.** Using "replace with" to switch a session
  to a `gh:` fresh checkout was refused with an error saying to pick a different session name because the git checkout
  path already exists. Not yet investigated: it may fail like that every time, or something subtler about that session's
  state may have triggered it. Reproduce first, then fix whichever it turns out to be."

Planning research found: Clone and Replace with share one composer path (`prefill_from` in
`crates/farhelm-ui/src/list/create_form.rs` copies title and folder verbatim; Replace with only adds which session it
replaces). Picking a repository changes only the destination, so the preview receives the copied title as explicit, and
`checkout_basename` in `crates/farhelm-proto/src/github_checkout.rs` maps it to the source's own directory; SPEC.md
(Fresh GitHub checkouts) says "An explicit name remains a conflict rather than gaining a suffix." So the second and
third entries are one bug, and it fails every time for an unrenamed gh: source of the same repository. The first entry
is specified behavior: SPEC.md says "Ordinary Clone and Replace start from the source session's actual directory; a new
checkout requires an explicit repository selection or a saved repository setup", and renaming never moves a checkout.

**The user's decisions (2026-10-02):**

- U1. "yeah so clone SHOULD [reuse] working copy by default thats why i was suspsicious. so yeah, the point is when you
  use "gh:" it means don't keep traeating the path as manually typed." Clone keeps sharing the source's working copy by
  default; the first entry is removed as working as intended.
- U2. When a fresh checkout is chosen and the title is still the copied one, unedited, treat the session as unnamed: its
  title and directory become the next free `repo-N` (the existing rule for an unnamed checkout).
- U3. The name field goes empty with the `repo-N` placeholder: "never show information that will be ignored".
- U4. Scope: fix Clone and Replace with the same way; remove all three entries. `farhelm agent clone` and the spec's
  rule that Clone keeps the working copy are untouched.
- U5. Review gate: a fresh-context Opus 5.5 reviewer at high effort, told to review adversarially. No review swarm.
- U6. No-workhorse mode: you do all the work yourself (see How to run).

**Binding repository constraints:**

- SPEC.md Fresh GitHub checkouts and Lifecycle operations (Clone, Replace, Replace with). Add, next to "An explicit name
  remains a conflict rather than gaining a suffix", that a title carried in by Clone or Replace with and left unedited
  is not an explicit name when a fresh checkout is the destination. Say that this also covers a source renamed after it
  was created and an ordinary session cloned into `gh:` (both now get `repo-N` where they used to get a name from the
  copied title), so it is not read as a regression. The Lifecycle operations text needs no change.
- The checkout preview's title must equal the create's, or the launch is refused as stale; see P2.
- Root `AGENTS.md`: a `fix` PR carries a changelog fragment and removes the TODO entries it addresses.

**Planner choices (from the planning review):**

- P1. Decide the title when it is read, not by changing state when a repository is picked. One pure helper (for example
  `fresh_checkout_title(text, edited, seed, checkout_mode)`) returns the empty string when the destination is a fresh
  checkout, the field is unedited and it holds a copied seed, and otherwise what `submitted_field` (in
  `crates/farhelm-ui/src/profiles.rs`) returns today. The empty string is already how an unnamed checkout is requested
  (`checkout_basename` treats it as absent, and a blank New form sends it today), so no API, helm or supervisor change.
  Switching back to a folder then restores the copied title with no extra code.
- P2. Every place in `create_form.rs` that reads the title for a launch goes through the helper: the preview request,
  the retry binding, the submit binding, the draft snapshot, and the re-read while the request key is made (every
  `submitted_field(&title...)` there). The ordinary-folder path keeps sending the copied title.
- P3. Key the helper on the destination being a fresh checkout, not on the gh: search handler, so all three ways in are
  covered: a repository from search, a saved repository setup from search recents, and the same from the history list.
  There is no typed-`gh:` submit path (submitting while search is in gh: scope is refused).
- P4. The field's display (U3) is derived the same way: empty while the helper would send empty, with the preview's
  basename as placeholder. Typing makes the title explicit again through the existing `title_edited` flag.
- P5. Tests: unit tests on the helper (unedited/edited/no seed, checkout/folder). Mounted tests in
  `e2e/tests/github-checkout-composer.spec.ts`, which already captures the preview's title without a network clone:
  Clone and Replace with each send an empty preview title that matches the create/replace body, the field shows empty
  with the `repo-N` placeholder, and an edited title is sent as typed. At most one real-clone test in
  `e2e/tests/github-checkouts.spec.ts`, for Replace with, whose helm path differs.

## Implementation outline

One PR (`fix:`), in `crates/farhelm-ui` only. Line numbers drift; find the code by name.

- `create_form.rs`: the helper (P1), its use at every read site (P2, P3), and the field display (P4), with doc comments
  saying why a copied title stops counting as typed once a fresh checkout is the destination (U1, U2) and why the field
  hides it (U3).
- Tests per P5.
- SPEC.md Fresh GitHub checkouts sentence. Check `website/` for a page describing Clone into a gh: checkout and update
  it if one does.
- Changelog fragment `kind: fixed`: cloning a session, or using Replace with, into a fresh GitHub checkout no longer
  fails with "a directory with that checkout name already exists".
- Remove the three TODO entries. Mark this plan's line in `plans/INDEX.md` `[executed]`.

### What not to build

No suffixing of copied titles, no proto/helm/supervisor change, no change to `farhelm agent clone`, no change to Clone's
default of sharing the working copy.

## How to run

### Files outside the repository

Per `plans/AGENTS.md` (Files outside the repository): this plan's working log is
`farhelm-plan-gh-clone-fresh-checkout-log.md` in the parent directory of the checkout you run in, derived as that
section says. Resolve it to an absolute path before you start. Scratch files go in the agent scratch directory root
`AGENTS.md` describes, not in the checkout.

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
- Commit message and PR title use Conventional Commits; the PR is `fix:`. It adds its changelog fragment under
  `releasing/changelog.d/` in the same commit, per root `AGENTS.md` (Releases and the changelog); validate with
  `python3 releasing/check-changelog.py format`.
- Run the commit message, PR title and PR description through the `scode-commit-msg-reviewer` skill's cold read. Leave
  the PR description empty when the diff and title say everything.
- The PR stays a draft. Never mark it ready and never merge; landing the plan stack is the user's job.

### Validation

Follow root `AGENTS.md` "Finishing work": the smallest set of checks likely to expose a regression, through
`scripts/record-test-run.py` for test execution. Typical choices, not a checklist: `cargo fmt --all -- --check`,
`cargo
clippy --all-targets -- -D warnings`, `cargo check -p farhelm-ui --features desktop`, the farhelm-ui unit tests
for `create_form.rs`, `github-checkout-composer.spec.ts` and the touched `github-checkouts.spec.ts` test on Chromium and
WebKit through the recorder, `dprint check` on changed Markdown, `python3 releasing/check-changelog.py format`, the
website build if a docs page changed, and `python -B scripts/check-test-sleeps.py` per `docs/test-sleep-check.md` with
`.agents/test-authoring.md` applied, since tests change.

### Review gate

Before finishing the PR, use the active galaxy-brain skill to delegate one review of its changes. The user demands a
fresh-context agent on Opus 5.5 at high effort, reviewing adversarially (U5), shelled out to the other harness if the
executing one cannot reach that model natively. No review swarm. The prompt carries the full charter, because the
reviewer has nothing else:

> Review the changes in this PR adversarially: assume there are defects and hunt for them. Cover general correctness
> (bugs, unhandled cases, broken invariants, wrong behavior against the acceptance criteria quoted below), design
> (whether the shape of the change fits the codebase and the goal, and whether a simpler shape would), and idiomatic
> code for the languages involved. Report findings to the named file, most severe first, each with the file, the quoted
> code, what is wrong, and the smallest fix. Edit nothing and touch no VCS state.

The prompt must also name the repository root (the executing checkout), the bookmark or commit range, the findings file
(in the scratch directory), and the acceptance criteria: The goal, U1-U6 and P1-P5. Include the full text of
`.agents/test-authoring.md` verbatim, as root `AGENTS.md` requires for test changes. Address what the reviewer finds
before moving on, and log the DECISION where you decline a finding. Do not write a launch command here or in the log;
the harness-shellout skill owns launch mechanics.

### Scope reassessment

Before implementing a substantial departure from the outline above (a proto, helm or supervisor change, state that is
rewritten when a repository is picked, suffixing copied titles, a change to `farhelm agent clone`; these are examples,
not a blacklist), and whenever the same component has needed repeated corrective review rounds, run a fresh-context
review through galaxy-brain with this charter, supplying the request, the user decisions above, this outline, the
current diff and the proposed departure (what changed, why it is necessary, and which simpler alternative was ruled
out):

> Identify where this plan does more than the user needs. For each substantial new mechanism, explain which requirement
> makes it necessary and whether an existing facility or narrower supported behavior would suffice. Propose the smallest
> coherent design. Challenge planner-authored requirements; identify any simplification that would change an explicit
> user requirement or repository constraint. Report concrete findings and alternatives, or say no unnecessary complexity
> was found. Do not manufacture objections or add safeguards to justify an unnecessary subsystem. Write findings to the
> supplied private file; change no product files or VCS state.

Reuse an existing review checkpoint when it can answer the question; do not schedule periodic reviews.

### Decision logging

Log each consequential choice in the working log as it happens, under a scannable `DECISION` label, with the
alternatives considered: in particular the helper's shape and name, the list of read sites it covers, how the field's
empty display and placeholder are derived, which docs page (if any) changed, and every reviewer finding you declined.
The user will ask for these later.

### Unattended fallback

Resolve routine implementation forks within the agreed scope and log them. Remove planner-invented machinery that turns
out unnecessary when the decisions still hold. A material scope expansion, a weakened guarantee, or an omitted required
behavior needs an agreed fallback or the user's decision; a review finding or a log entry is not authorization. Anything
that needs a decision: record the concrete tradeoff and block per `plans/AGENTS.md` (Executing, step 7).

## Done criterion

The plan is complete when its one draft PR exists on the plan stack's tip, satisfies the acceptance criteria, has passed
the review gate, has removed the three TODO.md entries, and has marked this plan's `plans/INDEX.md` line `[executed]`.
Open, not merged: merging is the user's job. Then close the plan per `plans/AGENTS.md` (Executing, step 8): write its
report, write a closing entry in its log, and stop the watchdog.

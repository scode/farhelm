# Host uninstall: say in plain words why it was refused and what to do

Written against main at 2897c914 on 2026-10-20. This is a goal file for an unattended run, executed through the planning
system in `plans/AGENTS.md`. You, the executing agent, have none of the conversation that produced it; everything you
need is here or in the repository files it names. SPEC.md and SPEC_impl.md stay authoritative over anything this file
says.

This plan has no dependency on another plan. It changes the helm's refusal text and the refusal's type, plus one docs
sentence; it changes no UI code. If you find a UI change unavoidable, log why, and note that `ui-peer-text-type.md`
converts the host dialogs' text and may need to land first (rebase onto it if it has).

## The goal

Uninstalling Farhelm from a remote host first checks, through the host's running supervisor, that none of its sessions
are still running. When a supervisor answers but this helm cannot use it, uninstall is refused. The refusal, shown in
the "Farhelm refused this action" dialog, pastes the state's internal name into the sentence ("while this host is
version-skew"), calls the problem "a supervisor answers there that Farhelm cannot use", and ends "Resolve that first"
without saying how. Replace it with one sentence per situation, in plain terms, each naming the way out, and give the
dialog its red **remove** button for these refusals.

TODO.md entry this plan covers (Near term): "**Rewrite the uninstall refusal for a host with a version mismatch.** Seen
2026-10-18 on the maintainer's 0.29.0 helm for a test host that a dev build had updated: the row read "too new", "the
host speaks protocol 60 (farhelm 0.0.0-unreleased); this helm speaks 51 (farhelm 0.29.0)" and "<host> runs a newer
farhelm than this helm; update this helm so the two speak the same protocol version". Choosing uninstall then printed,
in red: "uninstall is refused while this host is version-skew: a supervisor answers there that Farhelm cannot use, so it
cannot check that none of its sessions are running. Resolve that first, then choose uninstall again". The state's
internal name ("version-skew") is pasted into the sentence, "a supervisor answers there that Farhelm cannot use" is
vague, and "resolve that first" gives no way to resolve it. The same wording covers identity-mismatch,
identity-unverified and duplicate hosts. Name the problem in user terms (this host runs a newer or older Farhelm than
this helm) and the concrete way out for each case. Host refusals now appear in a dialog; this entry concerns their
wording."

### Acceptance criteria

**One sentence per situation.** The shared refusal for the four host states becomes five messages, because version skew
splits by direction. None contains an internal state name ("version-skew", "identity-mismatch", "identity-unverified",
"duplicate" as a token, "phase", "supervisor answers there"). Every one ends by pointing at removing the host from the
list as the way to stop using it without uninstalling, as SPEC.md says the layout refusals do; extend that SPEC.md
sentence (around SPEC.md:162-164) by one clause so it also covers these five refusals and their remove button. Match the
capitalization and final punctuation of the other uninstall refusals the dialog shows (the existing remove-instead
refusal starts with a capital and has no final period; the others are lowercase); log the choice. Drafts (final wording
is yours within these meanings; log it; values from the host are escaped as all peer text is):

- **Host older** (the host's protocol is lower; the row says "needs update"): "Farhelm can't uninstall from this host
  yet: it runs farhelm ‹host build›, which is too old for this helm (‹helm build›) to talk to, so Farhelm can't check
  whether any of its sessions are still running. Update the host first (its update button in the host list), then choose
  uninstall again. To stop using the host without uninstalling, remove it from the list instead."
- **Host newer** (the row says "too new"): "Farhelm can't uninstall from this host: it runs farhelm ‹host build›, which
  is newer than this helm (‹helm build›), and this helm can't talk to it, so it can't check whether any of its sessions
  are still running. Update this helm to farhelm ‹host build› or later, or uninstall from a helm that already runs that
  version. To stop using the host without uninstalling, remove it from the list instead." When the host's build does not
  read as a release version (a development build such as `0.0.0-unreleased`), drop the "to farhelm ‹host build› or
  later" target: "Update this helm, or uninstall from a helm that already runs that version."
- **Identity mismatch**: "Farhelm can't uninstall from this host: the Farhelm install at its ssh destination is not the
  one this entry was set up with (identity ‹recorded› on record, ‹reported› now), for example because the host was
  reinstalled or the destination now reaches another machine. If it is the machine you mean, choose adopt in the host's
  menu, then uninstall again; otherwise correct the destination in the host's settings. To stop using the host without
  uninstalling, remove it from the list instead."
- **Identity unverified**: "Farhelm can't uninstall from this host: something answers at its ssh destination but doesn't
  say which Farhelm install it is, so Farhelm can't confirm it is the host you set up or check its sessions. Farhelm
  keeps checking, and this clears once the host identifies itself; if the destination is wrong, correct it in the host's
  settings. To stop using the host without uninstalling, remove it from the list instead."
- **Duplicate**: "Farhelm can't uninstall through this entry: it reaches the same machine as ‹other entry's name›, so
  nothing is connected through this one. To remove Farhelm from that machine, choose uninstall on ‹other entry's name›,
  then remove this entry. If this entry should point somewhere else, change its ssh destination in its settings and
  press retry." The other entry is named by its display name, as the host list shows it; if it has been removed
  meanwhile, fall back to the way the UI names a missing twin ("another entry (host ‹id›)"). The helm sends no twin name
  today (the UI fills it in itself, `hosts::name_duplicate_twins` in the UI crate), so the helm needs its own small
  lookup of the twin's row and `host_display_name`.

**The remove button.** All five return the typed remove-instead refusal (`ProvisioningRequestError::RemoveInstead`, the
one the symlinked lib directory refusal already uses), so the dialog shows its red **remove** button next to **close**,
with no UI code change. No other action buttons (no update or adopt button in the dialog).

**The two neighbouring uninstall refusals** in the same function get the same treatment (plain words, a concrete way
out, no internal names), and stay plain refusals without the remove button:

- **Not connected**: today it pastes internal phase names ("it is unreachable-reprobing right now"). Draft: "Farhelm
  can't uninstall from this host while it isn't connected, because it has to check that none of its sessions are
  running. Once the host is connected, choose uninstall again." No state word: the host list's words live in the UI,
  copying them into the helm would drift, and the dialog sits over the row that already shows the state.
- **Identity differs on a connected host** (the fresh check reports a different identity, or one where none is on
  record, or none where one is): today it ends "resolve that first". Draft: "Farhelm can't uninstall from this host: the
  Farhelm answering at its ssh destination reports ‹reported, or "no identity"›, not ‹recorded, or "none"› on record for
  this entry, so Farhelm can't tell it is the same machine whose sessions it just checked. Choose retry in the host's
  menu and follow what the host list then says, then choose uninstall again."

**Also:**

- The helm's sentences do not contradict the advice the host row gives for the same state (the row's remedy line in
  `crates/farhelm-ui/src/hosts.rs`, `state_remedy`, and the helm's own `skew_remediation`). The duplicate sentence
  differs from the row on purpose, because it is about uninstalling rather than fixing the entry; that is expected, and
  the row is not changed.
- One sentence in the docs page `website/src/content/docs/docs/using/manage-hosts.mdx`, in the uninstall section (around
  76-83): a host that shows needs update, too new, an identity problem or a duplicate must be fixed first, as its row
  says, and **remove** still works for it. Per `website/AGENTS.md`, an unattended run starts no preview server; name the
  changed page in the report.
- A changelog fragment, `kind: changed` (or `fixed`; log which).
- The last code PR removes the TODO.md entry.

## Requirement sources

**The user's decisions (2026-10-20), from the planning page:**

- Too new: ""Update this helm to farhelm X or later, or uninstall from a helm that runs that version"", chosen over only
  "update this helm". Dropping "or later" for a development build was shown on the planning page and not questioned.
- Scope: "These five situations plus the two neighbouring uninstall refusals ("not connected", and "identity differs on
  a connected host")", chosen over the shared refusal only.
- Remove button: "Yes, for all five", chosen over only too new and identity unverified, and over text only.
- Direct remedy buttons: "Not now".
- Docs: "One sentence in the docs' Manage hosts uninstall section: fix a host's problem first, as its row says".
- Review gate and mode are in the run instructions below.

**Binding repository constraints:**

- SPEC.md, the host uninstall paragraph (around SPEC.md:146-169), including the layout refusals' "Removing the host from
  the list, which never touches the host, stays available whatever its layout, and the refusal points to it as the way
  to stop using the host without uninstalling", which this plan extends to the five refusals; the identity mismatch and
  duplicate passages (around 185-222); Update never downgrades (a too-new host is fixed by updating the helm).
- SPEC_impl.md's uninstall passage (around 4386-4398): a host that is not connected is refused when a supervisor answers
  there but cannot be used. Its rule is unchanged; only the words and the refusal's type change.
- Root `AGENTS.md`, "Changelog fragments" and "Tests and validation"; `.agents/test-authoring.md`; `website/AGENTS.md`
  before editing the docs page.

**Planner proposals (checked by a fresh-context planning review):** the outline below. A planner proposal is not a
requirement: a smaller design that meets the same acceptance criteria is fine, logged as a DECISION.

## Outline

Facts from the planning session's reading of main at 2897c914 (verify them):

- `uninstall_plan` in `crates/farhelm-helm/src/provisioning/service.rs` (docs around 1225-1244). The not-connected
  refusal is the `not_connected` closure (around 1252-1262), the identity-differs refusal is around 1292-1318, and the
  shared four-state refusal is around 1339-1351. The symlinked lib refusal returns `RemoveInstead` around 1413.
- `ProvisioningRequestError::RemoveInstead` is defined in `crates/farhelm-helm/src/provisioning/http.rs` (around 25) and
  mapped to the reply's typed remove flag around 272-297; the UI's `HostRefusalDialog` (`crates/farhelm-ui/src/hosts.rs`
  around 126-204) shows the remove button for it.
- `HostState` in `crates/farhelm-helm/src/manager.rs`:
  `VersionSkew { peer_protocol, peer_build, our_protocol, our_build, remediation }` (around 440-449),
  `Duplicate { twin, identity }` (around 528-534), `IdentityMismatch { recorded, reported }`,
  `IdentityUnverified { recorded }`; `phase()` around 565-576 is where the internal names come from. `skew_remediation`
  is around 4559-4590. Entry display names come from `host_display_name` (`crates/farhelm-helm/src/hosts.rs` around
  424). Peer text goes through `peer_text`.
- Tests: `uninstall_refuses_a_host_with_an_identity_problem` (`crates/farhelm-helm/src/provisioning.rs` around
  4255-4299) asserts the internal word "identity-mismatch" and must change; the identity-differs tests around 4300-4400
  assert "no identity"; the remove-instead tests around 4160-4195 are the model for asserting the typed refusal.

Proposed shape (planner proposal): one PR, `fix: say why uninstall was refused and how to get past it`, building each
sentence from the state's own fields in a small function per refusal (pure, unit-tested), returning `RemoveInstead` for
the five states; the neighbouring refusals reworded in the same PR; the docs sentence and fragment; the TODO.md entry
removed. Split the neighbouring refusals into a second PR if the first grows large; log the choice.

### Validation

Per root `AGENTS.md`, "Tests and validation", and `.agents/validation.md`: `cargo fmt --all -- --check`,
`cargo clippy --all-targets -- -D warnings`, and the helm crate's tests through the recorder; the website build check
`website/AGENTS.md` names for the docs edit; `python3 releasing/check-changelog.py format`.

New tests. The wording is a property of the pure sentence functions, so test it there, one case per situation: host
older (says to update the host), host newer (says to update this helm, names both builds, and drops the target for a
development build), identity mismatch (names adopt and settings), identity unverified (no adopt), duplicate (names the
other entry, and the fallback when it is gone), not connected and identity differs (plain words); each asserts that no
internal state name appears. The four states share one branch in `uninstall_plan`, so one path test covers the wiring:
update `uninstall_refuses_a_host_with_an_identity_problem` to assert the remove-instead kind and keep its no-operations
assertion. Update the existing neighbour tests to the new text. Add a duplicate path test only if the twin name lookup
lives outside the pure function. A browser test is optional: if `e2e/tests/provisioning.spec.ts` already drives a
version-skewed host's uninstall cheaply, assert the new sentence and the remove button there; otherwise log why it was
skipped.

## Fallbacks

No complexity gate. Departures that need the scope reassessment: changing which states are refused, adding action
buttons to the dialog, or changing UI code.

Decisions to log: the final wording of each sentence and its capitalization, how "reads as a release version" is
decided, the twin fallback wording, and the fragment's kind.

## How to run

### Files outside the repository

Per `plans/.agents/outside-files.md` (Files outside the repository): this plan's working log is
`farhelm-plan-uninstall-refusal-wording-log.md` in the parent directory of the checkout you run in, derived as that
section says. Resolve it to an absolute path before you start. Scratch files go in the agent scratch directory root
`AGENTS.md` describes, not in the checkout.

### Resume protocol

Your first action is to invoke the `agent-resumeable` skill with the log file's absolute path. If the log exists, read
it and resume where the previous session left off, cross-checking it against reality (the jj graph, bookmarks and open
PRs) rather than starting over. If it does not exist, this is a fresh start. A plan that an earlier executor worked on,
or that came back from review, also gets the resume check in `plans/.agents/executing.md` (Executing one plan, step 7)
before any work.

### No-workhorse

The user forbids sending any unit of your own decomposition of this work to a weaker model or a lower effort than your
own. The review gate and the scope reassessment review are not units of that decomposition: they keep the model and
effort this file demands for them. The user also forbids invoking, loading or following `scode-galaxy-brain` or
`scode-model-routing` at any point; the user chose this mode explicitly, so if galaxy-brain is already active when you
start, this file is the user's express instruction to stop it.

Sub-agents at your own model and effort are fine, and worth considering whenever they are likely to finish the work
sooner without costing quality: independent investigations run in parallel, or implementation split across agents.
Whether to use them is your call. Concurrent agents that might otherwise conflict can run in separate jj workspaces
whose results you merge back; you still own the stack and every PR. A sub-agent whose model is not available as a native
sub-agent type, the reviewer included, is launched through `$scode-harness-shellout`. The scope reassessment review runs
at your own model and effort. The sub-agents `plans/AGENTS.md` (Sub-agents this file requires) requires of every plan
(the resume check, and the cold reads of a blocked question and of the report) run as that file says.

### Resource watchdog

Immediately after the resume protocol, start a resource watchdog as a background process (not an agent) and keep it
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

- Split the work into a linear stack of reviewable PRs, one reviewable commit, stable bookmark and draft PR each, using
  the `jjstack` skill. The outline above suggests the slicing; err on the side of bite-sized PRs.
- The stack's base is `main@origin`, or this plan's own open PRs when it resumes, set up per
  `plans/.agents/executing.md` (Executing one plan, step 6); never another plan's PRs. Bookmarks are
  `plan/uninstall-refusal-wording/<nn>-<short-name>` (for example `plan/uninstall-refusal-wording/01-refusal-text`).
- Avoid churn: code added in one PR and deleted in a later one means the stack should have been shaped differently.
  Within this run, a PR that needs correcting is restructured rather than corrected on top, and that applies to PRs an
  earlier run built too.
- Commit messages and PR titles use Conventional Commits, with changelog fragments per root `AGENTS.md` (Changelog
  fragments): a later PR that extends an earlier PR's still unreleased change edits that fragment rather than adding a
  second one.
- The last code PR removes the TODO.md entry this plan covers (it ends with this plan's `Plan:` reference). TODO.md is a
  shared bookkeeping file: on a rebase conflict there, keep both sides.
- Run every commit message, PR title and PR description through the `scode-commit-msg-reviewer` skill's cold read.
- Before every push of a plan bookmark, run `queue status` and confirm the line still carries your claim id
  (`plans/.agents/executing.md`, Executing one plan, step 9).
- Never edit `plans/`: the plan's state moves only through `scripts/plans-queue.py`, as `plans/AGENTS.md` describes.
- PRs stay drafts. Never mark one ready and never merge; landing is the monitor's job (`plans/AGENTS.md`).

### Review gate

Before finishing each PR, launch a review of its changes yourself and address what it finds before moving on. The user
demands exactly this reviewer, and no review swarm: a single fresh-context agent on gpt-6.1-sol at high effort, launched
through `$scode-harness-shellout` when your harness cannot serve that model natively (that skill owns the launch
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
- the acceptance criteria: this file's goal, the user's decisions, and the outline.

When a PR changes tests or their fixtures, include the full text of `.agents/test-authoring.md` verbatim, as root
`AGENTS.md` requires.

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

Reuse an existing review checkpoint when it can answer the question; do not schedule periodic reviews. Same-model
sub-agents you choose to run get bounded units within the outline; an unapproved new mechanism is outside their task,
and one discovered mid-unit is left unimplemented and reported back to you. Apply the same decision boundary as
Unattended fallback below.

### Decision logging

Log each consequential choice in the working log as it happens, under a scannable `DECISION` label, with the
alternatives considered: in particular those named under Fallbacks above, and every review finding you decided not to
follow. The user will ask for these later.

### Unattended fallback

Resolve routine implementation forks within the agreed scope and log them as DECISIONs. Remove planner-invented
machinery that turns out to be unnecessary, as long as the user's decisions and the repository constraints still hold.
Wording in this file that turns out inaccurate (a line number, a file name) is corrected to the truth; the plan-specific
agreed fallbacks are under Fallbacks above.

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

The plan is complete when a linear stack of open draft PRs exists that together meets the acceptance criteria above,
each PR passed its review gate, and the last code PR removes the TODO.md entry. Open, not merged. If a `## Decisions`
section exists, its latest entry must also be satisfied. Then close the plan per `plans/.agents/executing.md` (Executing
one plan, steps 11 and 12): deliver its report through the queue script, write a closing entry in its log, and stop the
watchdog. Never edit `plans/` yourself.

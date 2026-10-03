# Say plainly that an unreleased helm has no release payloads to download

Written against main at cd8bd112 on 2026-10-02. This is a goal file for an unattended run, executed through the planning
system in `plans/AGENTS.md`. You, the executing agent, have none of the conversation that produced it; everything you
need is here or in the repository files it names. SPEC.md and SPEC_impl.md stay authoritative over anything this file
says.

## The goal

A release-shaped helm built from main (version `0.0.0-unreleased`) that sets up or updates a host without a payload
source today tries to download the payloads of release `v0.0.0-unreleased`, gets a 404, and tells the user "the release
is not published or is still publishing; retry in a few minutes, or pass --payload-dir". Retrying never helps: no
release will ever carry that version. After this plan it refuses before any download with a message saying this helm is
an unreleased build, so no published release carries its payloads, and to pass `--payload-dir` with payloads built from
the same commit. The TODO entry this replaces is removed.

Acceptance criteria:

- A release-shaped build whose version is a development version (`0.0.0` with any prerelease) and whose payload
  selection is the default refuses when a payload is first requested, before any network request and before any host
  change, with the new message. Helm startup is unaffected.
- `--release-base-url` is not refused on any build (D18: selectable on any build), and the ordinary 404 message for real
  versions (D17, shared with `install.sh`) is unchanged.
- A real version still downloads from the GitHub release matching it, exactly as today.
- The tests and prose in the outline are updated; the TODO.md entry is removed.

## Requirement sources

**The user's request:** "let's plan the following, each in its own plan: ... provisioning protocol guard", for the
TODO.md `Near term` entry, verbatim as of cd8bd112: "Guard provisioning against pushing payloads older than the helm's
own protocol. A release-shaped helm built from a commit newer than the latest release (the local stable-binary flow does
exactly this) provisions remote hosts with DOWNLOADED released payloads by default (D13), so the freshly provisioned
supervisor can speak an older protocol than the helm that just installed it — and the helm then refuses it at the hello
gate. Nothing is damaged (the refusal is the version rule working), but the failure arrives one step late, as a skewed
host instead of a refused provisioning attempt. Possible shapes: compare the payload's version against the helm's
`PROTOCOL_VERSION` before pushing and refuse with a message naming the mismatch; or make the staged-payload path
(`--payload-dir`, `FARHELM_HELM_PAYLOAD_DIR`) the documented answer for from-main helms. Noted 2026-08-31 when upgrading
the stable install to a from-main build while the newest release was still 0.1.1."

**What the user was told and decided (2026-10-02):**

- Since #1223 (2026-09-29) main's version is the sentinel `0.0.0-unreleased`, so a from-main helm can no longer download
  an older release; the entry's scenario does not occur. The one remaining way to push older payloads is a staged
  `--payload-dir`, whose payloads carry no version (a pre-push comparison was ruled out before, see
  `TRIAGE_OUTCOMES.md`), and the separate plan `plans/queue/attach-refusal-reason.md` makes that case fail at once
  naming the protocol mismatch. The 404 message for the sentinel case is misleading.
- U1. The user chose: "A small plan that makes the download failure say plainly that this build is unreleased, so there
  is nothing to download, and that you should pass `--payload-dir`. It also removes the TODO entry." ("(a)")
- U2. Review gate: a fresh-context Opus 5.5 reviewer at high effort. No-workhorse mode (see How to run).

**Binding repository constraints:**

- SPEC_impl.md D13 (payload sources, ~2799-2814), D17 (the shared 404 message), D18 (`--release-base-url` on any build),
  and "Version and skew", the sentinel paragraph (~2925-2942), which today says the download "names a release that does
  not exist and fails".
- Root `AGENTS.md`: a `fix` PR carries a changelog fragment; SPEC changes go with the behavior.

**Planner choices (from the planning review):**

- P1. Refuse lazily, in a payload source whose `path()` fails, like the existing developer-build refusal `NoPayloads`
  (crates/farhelm-helm/src/provisioning/payloads.rs). Never at construction: an error from `production_payloads`
  propagates to helm startup, so every release-shaped build from main, the desktop app included, would refuse to start.
  `prepare_payloads` (provisioning/service.rs) already resolves every payload before any host action, so a refusal from
  `path()` is already before the first host change.
- P2. One more guarded arm in the selection match in `production_payloads_with_key` ("This function IS the policy"),
  e.g. `PayloadSelection::Default if is_development_build(&version) => …`, returning a sibling of `NoPayloads` (or
  `NoPayloads` carrying which message); nothing in `release_payloads.rs`. The new message must say "unreleased" and
  "payloads built from the same commit", not "release files" (a published release's files are exactly the older-protocol
  payloads the original entry warned about).
- P3. Key the check on the `version` argument `production_payloads_with_key` already takes, and build the default URL
  from that same argument (`default_release_base_url` takes the version instead of reading `env!`). Production still
  passes `release_payloads::VERSION`. Without this, no test can cover the release-download default on a build from main,
  and `production_payloads_selects_by_payload_selection_and_release_build` would fail (it asserts a download from
  `v{CARGO_PKG_VERSION}`, which is the sentinel in every test build) and could not simply be flipped (the release gate
  runs it on a real version).
- P4. Extract the development-build predicate (inline in `build_is_newer`, crates/farhelm-helm/src/hosts.rs) as one
  small function used by both. It is `0.0.0` with any prerelease, not the literal `0.0.0-unreleased`.
- P5. Changelog fragment `kind: none`, with the reason as its body: only builds of main carry the sentinel, so no
  published release takes this branch. `fix:` stays the right type (a user-visible message for anyone running a build
  from main).

## Implementation outline

One PR, `fix:`. Line numbers drift; find code by name.

- payloads.rs: P2-P3; hosts.rs: P4.
- Tests: through `production_payloads_with_key`, a sentinel version with the default selection on a release-shaped build
  gives the exact new message from `path()`; a non-sentinel version (`FIXTURE_VERSION`) still gives the GitHub
  `ReleasePayloadSource` at that version (identify it through `Debug`, as the existing selection test does). Update
  `production_payloads_selects_by_payload_selection_and_release_build` accordingly. No fixture server: the new source
  makes no request by construction. Existing `--release-base-url` tests stay as they are.
- Prose: the SPEC_impl.md sentinel paragraph (a refusal before any download that names `--payload-dir`; and that
  `--release-base-url` is not refused, so the asymmetry reads as deliberate), the D13 payload-sources paragraph (the
  sentinel exception or a pointer), the root `Cargo.toml` comment ("finds no such release and fails"),
  `default_release_base_url`'s and `PayloadSelection::Default`'s docstrings, and `build_is_newer`'s if the predicate
  moves.
- The commit message says why the original guard is no longer needed (the sentinel version, and the attach-refusal plan
  covering staged payloads). Remove the TODO.md entry. Changelog fragment per P5.

### What not to build

No version threading through staged payloads; no protocol comparison before pushing; no change to D17's 404 text or to
`install.sh`; no refusal on `--release-base-url`.

## Plan-specific notes

### Validation

Likely relevant: `cargo fmt --all -- --check`, both clippy runs, the helm crate's payloads and provisioning selection
tests through the recorder (`--tmux none`), `python3 releasing/check-changelog.py format`, and `dprint check` on changed
Markdown.

### Decisions to log

The exact message text, the source type's shape (sibling struct or `NoPayloads` variant), where the predicate lives, and
any reviewer finding you declined.

## How to run

### Files outside the repository

Per `plans/AGENTS.md` (Files outside the repository): this plan's working log is
`farhelm-plan-unreleased-download-message-log.md` in the parent directory of the checkout you run in, derived as that
section says. Resolve it to an absolute path before you start. Scratch files go in the agent scratch directory root
`AGENTS.md` describes, not in the checkout.

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
  `plan/unreleased-download-message/<nn>-<short-name>`.
- One commit, bookmark and draft PR per PR in the implementation outline, as a linear stack in the order given. Err on
  the side of bite-sized PRs, but do not create churn: code added in one PR and deleted in a later PR of the same stack
  means the stack should have been shaped differently. Within this run, a PR that needs correcting is restructured
  rather than corrected on top, and that applies to all of this plan's open PRs, including ones an earlier run built.
- Commit messages and PR titles use Conventional Commits with the types the outline names. A `feat`, `fix`, `perf`,
  `style` or `revert` PR, or one whose type carries `!`, adds its changelog fragment under `releasing/changelog.d/` in
  the same commit, per root `AGENTS.md` (Releases and the changelog); validate with
  `python3 releasing/check-changelog.py format`.
- Run every commit message, PR title and PR description through the `scode-commit-msg-reviewer` skill's cold read. Leave
  a PR description empty when the diff and title say everything.
- PRs stay drafts. Never mark one ready and never merge; landing waits until the maintainer has reviewed this plan's
  report (`plans/AGENTS.md`). Never edit `plans/` in a PR; this plan's state moves only through
  `scripts/plans-queue.py`, as `plans/AGENTS.md` describes.

### Validation

Follow root `AGENTS.md` "Finishing work" for each PR: the smallest set of checks likely to expose a regression, through
`scripts/record-test-run.py` for test execution, with the narrow-tests recipe in `.agents/narrow-tests.md` for
reproductions. When tests or their fixtures change, apply `.agents/test-authoring.md` and run
`python -B scripts/check-test-sleeps.py` per `docs/test-sleep-check.md`. `dprint check` on changed Markdown. The
plan-specific notes above list the checks this work most likely needs; they are suggestions, not a checklist. Report
checks run, reused (with the covered revision) and skipped (with the reason) in the report.

### Review gate

Before finishing each PR, use the active galaxy-brain skill to delegate one review of its changes. The user demands a
fresh-context agent on Opus 5.5 at high effort, shelled out to the other harness if the executing one cannot reach that
model natively. No review swarm. The prompt carries the full charter, because the reviewer has nothing else:

> Review the changes in this PR adversarially: assume there are defects and hunt for them. Cover general correctness
> (bugs, unhandled cases, broken invariants, wrong behavior against the acceptance criteria quoted below), design
> (whether the shape of the change fits the codebase and the goal, and whether a simpler shape would), and idiomatic
> code for the languages involved. Report findings to the named file, most severe first, each with the file, the quoted
> code, what is wrong, and the smallest fix. Edit nothing and touch no VCS state.

The prompt must also name the repository root (the executing checkout), the bookmark or commit range, the findings file
(in the scratch directory), and the acceptance criteria: this file's goal, the user's request and decisions, and the
planner choices. When the PR changes tests, include the full text of `.agents/test-authoring.md` verbatim, as root
`AGENTS.md` requires. Address what the reviewer finds before moving on, and log the DECISION where you decline a
finding. Do not write a launch command here or in the log; the harness-shellout skill owns launch mechanics.

### Scope reassessment

Before implementing a substantial departure from the outline above (new persistent state, a new endpoint, protocol
message or subsystem, a compatibility layer, or anything the "What not to build" list names; these are examples, not a
blacklist), and whenever the same component has needed repeated corrective review rounds, run a fresh-context review
through galaxy-brain with this charter, supplying the request, the user decisions above, this outline, the current diff
and the proposed departure (what changed, why it is necessary, and which simpler alternative was ruled out):

> Identify where this plan does more than the user needs. For each substantial new mechanism, explain which requirement
> makes it necessary and whether an existing facility or narrower supported behavior would suffice. Propose the smallest
> coherent design. Challenge planner-authored requirements; identify any simplification that would change an explicit
> user requirement or repository constraint. Report concrete findings and alternatives, or say no unnecessary complexity
> was found. Do not manufacture objections or add safeguards to justify an unnecessary subsystem. Write findings to the
> supplied private file; change no product files or VCS state.

Reuse an existing review checkpoint when it can answer the question; do not schedule periodic reviews.

### Decision logging

Log each consequential choice in the working log as it happens, under a scannable `DECISION` label, with the
alternatives considered: in particular the ones the plan-specific notes above name, and every reviewer finding you
declined. The user will ask for these later.

### Unattended fallback

Resolve routine implementation forks within the agreed scope and log them. Remove planner-invented machinery that turns
out unnecessary when the decisions still hold. A material scope expansion, a weakened guarantee, or an omitted required
behavior needs an agreed fallback or the user's decision; a review finding or a log entry is not authorization. Anything
that needs a decision: push what is consistent, record the concrete tradeoff, continue independent authorized work, and
block per `plans/AGENTS.md` (Executing one plan, step 10) for what depends on it.

## Done criterion

The plan is complete when every PR in the implementation outline exists as an open draft, the stack satisfies the
acceptance criteria, every PR has passed the review gate, and the last code PR has removed the TODO.md entry this plan
covers. Open, not merged: merging happens only after the maintainer has reviewed this plan's report. If a `## Decisions`
section exists, its latest entry must also be satisfied. Then close the plan per `plans/AGENTS.md` (Executing one plan,
steps 11 and 12): deliver its report through the queue script, write a closing entry in its log, and stop the watchdog.
Never edit `plans/` yourself.

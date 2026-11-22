# Report each host setup command's result in its own output

Written against main at d6157939f on 2026-11-21. This is a goal file for an unattended run, executed through the
planning system in `plans/AGENTS.md`. You, the executing agent, have none of the conversation that produced it;
everything you need is here or in the repository files it names. SPEC.md and SPEC_impl.md stay authoritative over
anything this file says, and where this plan changes what they specify, it says so and you update them in the same
stack.

## The goal

This plan covers one TODO.md entry (bucket "Near term"). Verbatim:

> **Report remote command status in-band, not through ssh's exit status.** Tailscale SSH on a Mac reports exit status 0
> for every command (tailscale/tailscale#18256, still open on 2026-11-17), so adding such a Mac fails at the first probe
> ("the supervisor probe closed before hello completion with exit status exit status: 0"), and past that a failed
> install step, a payload checksum mismatch included, would read as success. Have every provisioning command Farhelm
> runs on a host end with a status record carrying a per-run random tag, take the status from that record, and treat a
> missing or malformed record as failure with a clear error; keep ssh's own 255 as an exit status. The commands share
> one funnel, so this is a contained change. Decided 2026-11-17 over detecting Tailscale and refusing the host. Finish
> with a real end-to-end add of a Mac that has Tailscale SSH on, since Tailscale SSH on macOS has other open problems.
> Findings, upstream status and every place that reads exit statuses:
> `lore/2026-11-17-tailscale-ssh-macos-exit-status.md`.

Read that lore file in full before starting. It is the diagnosis and the decision record.

### Where it stands (from planning research at main d6157939f; line numbers approximate)

- **The funnel.** `SystemBackend::remote_command` in `crates/farhelm-helm/src/provisioning/backend.rs` (about 814)
  prefixes `REMOTE_RUNTIME_PREFIX` (about 1665) to the script; the route turns it into `sh -c <script>`
  (`provisioning/access.rs`, about 152), quoted as one word for ssh (about 366). Container hosts reached through
  `podman exec … sh -c` take the same path (about 230). Its production callers:
  - `run_shell` (about 921), which covers every `require_shell` and `metadata_on_target` call and the Tensorlake
    provisioning calls in `provisioning/tensorlake_backend.rs`;
  - `remote_put` (about 1511), the payload upload;
  - `spawn_probe` (about 1642), the first check the add flow runs;
  - `mac_reboot_shell` in `provisioning/tests/macos_real_tests.rs`, test-only: it needs no resolving, but must compile
    against the new return type. Local targets do not use it (about 916).
- **Out of scope, deliberately.** The live connection (`transport.rs`, about 275) runs `farhelm internal stdio` and
  speaks Farhelm's own protocol; its exit status only explains a failed connection (127 reads as "Farhelm is not on that
  host"). Container-place Podman commands (`environment/podman_probe.rs`, `environment/podman.rs`), Tensorlake snapshot
  build scripts (`tensorlake/shell.rs`), and the ssh sign-in master (`ssh_sign_in.rs`) do not go through the funnel and
  stay as they are.
- **Output capture.** Both streams are fully buffered, capped at 64 KiB (`MAX_CHILD_STREAM_BYTES`, about 51) by
  `drain_capped` (about 1923); `finish_child_output` (about 2202) builds the result and is shared with callers outside
  the funnel (`podman_probe.rs`, `tensorlake/build_script.rs`), so the record must not be parsed there. Stdout carries
  data in many callers: binary `dd` chunks (`tensorlake_backend.rs`, about 161), NUL-separated records, an exact
  `b"Darwin\n"` match, the sandbox home path as the whole of stdout, and the probe's protocol frames. Stderr is always
  drained and used only for diagnostics and a few classifiers (`nested_session_failure`, Tensorlake `classify`,
  `linger_was_refused`). The upload's stdout is size reports (`drain_transfer_sizes`, about 2054). The probe's stderr is
  read line by line for marker records (`ProbeStderr::observe_line`, about 1828).
- **Exit codes that carry meaning,** all checked in `backend.rs` and `tensorlake_backend.rs`: 75 (Farhelm not installed,
  the probe; also an absent agent file on Tensorlake), 44 (inspected path missing), 76 (uploaded payload's checksum
  mismatch), 78 and 79 (destination changed under the upload), 42 (no `gh`), 81 and 82 (directory removal refusals), 3
  and 4 (Tensorlake: no systemd, no passwordless sudo), 255 (ssh's own failure).
- **The probe.** It prints `farhelm-probe-command-started-v1` on stderr, then either `exec`s `farhelm internal stdio`
  (which itself exits 75 when no supervisor runs, SPEC_impl.md about 2642) or exits 75. Once the hello completes, the
  probe is stopped by killing ssh, so no status record can arrive on that path.
- **Test launchers.** About ten test launchers substitute their own script for the real one and exit with a chosen code
  (`ScriptLauncher` in `provisioning.rs` alone has several uses; others in `backend.rs` tests, `snapshot_preparer.rs`,
  `tensorlake_backend.rs`, whose exact-text assert also changes). Once the funnel wraps commands, they all compile and
  then fail at runtime with "missing record". `provisioning/ssh_stand_in.rs` holds the shared stand-in machinery
  (`spawn_remote`).
- **Real ssh coverage** stays with `provisioning_and_update_over_ssh_preserve_an_operable_session` (`provisioning.rs`,
  about 14366), on localhost and in the CentOS release gate.
- `uuid` is already a helm dependency, and `Uuid::new_v4().simple()` is already used (`environment/podman.rs`, about
  344).

### Acceptance criteria

1. Every command that goes through `remote_command` ends by printing, on stderr, one record line
   `farhelm-command-status-v1 <tag> <status>`, where `<tag>` is a fresh random value per invocation and `<status>` is
   the script's real exit status. The script runs in a child shell, so a script that calls `exit` partway, sets its own
   EXIT trap, or `exec`s still produces the record (the probe's success path excepted, criterion 4).
2. `run_shell` and `remote_put` resolve the status from the record, through one shared resolver, right after capture and
   before anything reads the code (`nested_session_failure`, and `remote_put`'s check that cancels the upload's input
   feeder):
   - ssh's own 255 stays 255, and the record is ignored.
   - Otherwise the line carrying the marker, the matching tag and a code from 0 to 255 supplies the code, and is
     stripped from stderr together with the newline the wrapper printed before it (`require_shell_within` branches on an
     empty stderr). A line with another tag, or one that only echoes the tag, is ordinary output.
   - No valid record is a failure with a clear error: the host did not report how the command ended; some ssh servers,
     Tailscale SSH on a Mac among them, do not report exit statuses. It includes ssh's own status and the stderr. When
     the transport reported something other than 0 (a failed `podman exec`, say), the error does not name Tailscale.
   - Existing checks for 0, 44, 75, 76, 79 and the rest keep working unchanged, because the code they see comes from the
     record.
3. One pure parser recognizes a record line for a given tag, used by the resolver and by the probe's stderr reader. It
   does not reuse or change the existing untagged probe markers, which have a different job.
4. The probe: its stderr reader recognizes the record and keeps it out of diagnostic text. On the paths where the hello
   completed, no record is needed: the hello is the proof. On the failure path, the record's code replaces ssh's (75 →
   Farhelm absent, including the 75 the exec'd stdio proxy reports with no supervisor); the 255 check stays on ssh's
   real status; a missing record gives criterion 2's error, not the old "closed before hello completion with exit status
   0" text.
5. Tests, without sleeps (`.agents/test-authoring.md`):
   - parser and resolver unit tests: a missing record, a forged record with the wrong tag ignored, an echo of the tag
     alone ignored, a malformed code, 255 passed through, the record and its newline stripped;
   - a lying ssh stand-in that runs each command honestly but always exits 0, built on the existing stand-in machinery,
     showing that a failing `require_shell` still fails, that an upload whose bytes do not match is still refused with
     76, that the probe on a host without Farhelm still reads as absent, and that a stand-in that drops the record gets
     criterion 2's error;
   - the existing test launchers that substitute a script re-wrap it with the production wrapper through one shared
     helper beside `ssh_stand_in::spawn_remote`, rather than each being edited by hand.
6. SPEC_impl.md, "Provisioning", gains a paragraph on the status record (why, what it is, what a missing one means);
   SPEC_impl.md's sentences about the probe's 75 and the live connection's exit-code explanations (about 2642-2644 and
   2667-2671) say what holds now, including that a live connection that fails to start over a server that loses exit
   statuses gets a vaguer explanation. Add a sentence to SPEC.md, "Supported host setup", if it describes how setup
   results are judged.
7. Tensorlake sandbox setup goes through the same wrapper, and is confirmed on one real Tensorlake sandbox (Decisions).
8. The PR carries a changelog fragment and removes the TODO entry.

## Decisions already made

Settled with the maintainer on the 2026-11-21 planning page:

- **The real Mac test is the maintainer's, after the plan lands.** The plan finishes without an end-to-end add of a Mac
  with Tailscale SSH on; nothing in the repository gives an unattended executor such a Mac. The report's "things you
  should know" tells the maintainer exactly how to try it from a build of the change (adding the Mac through the desktop
  app, or the ignored real-macOS provisioning test in `provisioning/tests/macos_real_tests.rs` pointed at the Mac's
  Tailscale address), and what to watch for: Tailscale SSH's "check mode" asking for an interactive re-approval, and
  sessions left behind on disconnect (tailscale/tailscale#21150). Anything found there becomes a follow-up.
- **Tensorlake sandbox setup is included, and may be checked on one real sandbox.** The maintainer allowed one real
  Tensorlake sandbox, deleted afterwards, to confirm that the record survives Tensorlake's ssh gateway (a gateway that
  altered stderr would make every sandbox setup fail). This is the request `.agents/tensorlake.md` requires before an
  agent uses real sandboxes: at most one sandbox at a time, named and recorded the way that file says, every sandbox and
  snapshot you create removed before you close. Run the exact wrapped command shape the helm sends, through the gateway
  the way the helm reaches a sandbox, and confirm the record arrives intact for a succeeding and a failing command. If
  no Tensorlake account or key is available to you, skip the check and say so in the report; do not block.
- **Review gate:** a single fresh-context `gpt-6.2-sol` agent at `high` effort.
- **Release note:** the maintainer wrote their own entry (the `## Release note` section).
- **No-workhorse mode:** the plans default, stated on the planning page.

## Requirement sources

**The maintainer's request and decisions:** the TODO entry, the lore file's decision (in-band status over detecting and
refusing Tailscale), and the decisions above.

**Binding repository constraints:**

- SPEC_impl.md, "Provisioning", and the probe and live-connection passages named in the acceptance criteria.
- `.agents/test-authoring.md` for every test added or changed; `python -B scripts/check-test-sleeps.py` when Rust tests
  change.
- `.agents/tensorlake.md` for the real sandbox.
- Root `AGENTS.md`, "Conventional Commits", "Changelog fragments", "Tests and validation", "The live install is
  off-limits", and its "Instructions by path" for every path you touch.

**Planner proposals** (from research and an independent planning review):

- The record on stderr, not stdout: stdout carries data in too many callers.
- One resolver for both callers, applied after capture and outside `finish_child_output`; one parser shared with the
  probe. The probe's streaming stderr reader and the capped drain stay separate, since they have different contracts.
- A wrapper of the shape
  `sh -c '<prefix><script>' farhelm; s=$?; printf '\n%s %s %s\n' farhelm-command-status-v1 <tag> "$s" >&2; exit "$s"`.
  The final `exit "$s"` keeps honest servers and existing stand-ins reporting real statuses. A subshell `( … )` instead
  of a child `sh -c` is a close call: it avoids a second quoting layer and keeps the inner script verbatim, but turns a
  script syntax error into "did not report" rather than a status. Choose after looking at the quoting code, and log the
  choice as a DECISION.
- Rejecting a second valid record is optional, since the tag cannot be guessed; decide and log.
- Remote signal deaths used to come back as ssh's 255 (`ssh_stand_in.rs` docs) and will now come back as 128+n from the
  record. Accept that; note it in the report.
- A "44 → no file" scenario through the lying stand-in adds little over a failing `require_shell`; include it only if it
  costs nothing extra.
- Fragment, `kind: fixed`; its first paragraph can follow the maintainer's release note.

## Outline

1. `plan/in-band-host-command-status/01-command-status-record`: the wrapper, tag, parser and resolver, the probe change,
   the shared re-wrap helper for test launchers, the lying stand-in tests, the spec text, a changelog fragment, and the
   TODO entry's removal. `fix:` (for example `fix: read host setup results from the command, not ssh's exit status`).
   One PR, because a split would leave the first PR without the tests that prove the fix while still carrying every
   test-launcher change to keep the suite passing.

## Fallbacks

- If you find a provisioning command that reads its result from ssh's exit status without going through
  `remote_command`, route it through the funnel when that is a small change; otherwise log it and report it as a
  follow-up. Commands outside provisioning (listed above as out of scope) stay as they are.
- If a stand-in or real test depends on stderr being exactly empty or exactly some text, make the resolver's stripping
  exact rather than adapting the test to stray output.
- If the real Tensorlake check shows the gateway mangles the record, do not add a Tensorlake-specific bypass on your
  own: block with the evidence, after pushing what is consistent.
- A reviewer finding that asks for Tailscale detection, a fallback to trusting ssh's exit status, or in-band status on
  the live connection is declined by decision; log it.

## How to run

### Files outside the repository

Per `plans/.agents/outside-files.md`, "Files outside the repository": this plan's working log is
`farhelm-plan-in-band-host-command-status-log.md` in the parent directory of the checkout you run in, derived as that
section says, and the watchdog's status file is `farhelm-plan-in-band-host-command-status-watchdog.json` beside it.
Resolve both to absolute paths before you start; this file does not create either. Scratch files go in the agent scratch
directory that `AGENTS.md`, "Agent scratch space", describes, not in the checkout.

### Builds and tests

Run builds and tests as `.agents/validation.md` describes, every runtime test through
`python3 scripts/record-test-run.py`, and report checks run, reused and skipped per root `AGENTS.md`, "Tests and
validation". Choose the narrowest checks that exercise what the PR changes; the localhost ssh provisioning test above is
the real-ssh check. Apply `.agents/test-authoring.md` to every test you add or change, and run
`python -B scripts/check-test-sleeps.py` as `docs/test-sleep-check.md` prepares it when Rust tests change. Never take a
state-changing action against the live Farhelm install on this machine (root `AGENTS.md`, "The live install is
off-limits").

### Resume protocol

Your first action, after confirming that the checkout you run in is a checkout of `scode/farhelm` (the nearest Git
worktree or jj workspace containing your working directory, one of whose remotes points at that GitHub repository; if
not, stop, write nothing, and say so), is to invoke the `agent-resumeable` skill with the log file's absolute path. If
the log exists, read it and resume where the previous session left off, cross-checking it against reality (the jj graph,
bookmarks and open PRs) rather than starting over. If it does not exist, this is a fresh start. A plan that an earlier
executor worked on, or that came back from review, also gets the resume check in `plans/.agents/executing.md`,
"Executing one plan" (step 7) before any work.

### Execution mode: no-workhorse

Never send a unit of your own decomposition to a weaker model or a lower effort than your own. Never invoke, load or
follow `scode-galaxy-brain` or `scode-model-routing` during this plan. No-workhorse is the mode
`plans/.agents/planning.md` sets for plans, and the maintainer saw it stated while planning, so it is an explicit
choice: if galaxy-brain is already active when you start, this file is the instruction to stop it.

Sub-agents at your own model and effort are fine, and worth considering when they are likely to finish the work sooner
without costing quality. Concurrent agents that might otherwise conflict can work in separate jj workspaces whose
results you merge back; you still own the stack and every PR. Launch a sub-agent natively, with model and effort set
explicitly where the mechanism allows; when the model you need is not available as a native sub-agent type (the reviewer
below, for instance), `$scode-harness-shellout` supplies the launch mechanics.

The review gate keeps the model and effort this file demands; the scope reassessment runs at your own model and effort.
The sub-agents `plans/AGENTS.md`, "Sub-agents this file requires", requires of every plan (the resume check, and the
cold reads of a blocked question and of the report) are exempt from the rules above and run as that file says: native,
read-only, at your own model.

### Resource watchdog

Right after the resume protocol, and before the first sub-agent, build or test run, start the `scode-build-goal` skill's
watchdog and keep it running for the whole plan. It is the Deno script `scripts/watchdog` in that skill's directory;
resolve its path with `realpath` so it is the same file whichever harness runs you, and read its module docs. Its status
file is the one named under "Files outside the repository" above. Watch, each as a `--path` of `sample`: the root of the
checkout you run in, the home directory, `/tmp`, the directory holding the status file, your scratch directory, and any
jj workspace or build directory you later create on another filesystem (restart the sampler with it added). Sample every
60 seconds, at the script's default alert levels.

- In Claude Code, run `watchdog observe --status <file>` under the Monitor tool, then
  `watchdog sample --status <file> --path <dir>...` under a second Monitor, both at the maximum timeout, then run
  `watchdog check --status <file>`. Every line either prints is a notification. Whenever either is re-armed, stop the
  other and arm both again, observer first, then `check`. An expiry notice is routine; a monitor exit is a monitoring
  failure.
- In Codex, arm `watchdog observe --status <file> --exit-on-event` and then the sampler as two separate runs through the
  `skillette-codex-wake-on-exit` mechanism (load the `skillette` skill and follow that entry), then run `check`. On the
  observer's callback, acknowledge it and arm a fresh observer; after a `STALE`, also stop the hung sampler through its
  handle and arm a new one; then run `check` and act.
- Before starting the real monitors in each session, prove the mechanism on throwaway status files in your scratch
  directory with `--interval 5`: a sampler run with `--synthetic-alert` delivers an `ALERT`; a sampler killed through
  the `pid` in its status file is reported; a sampler run with `--stall-after 2`, watched by its own observer armed
  first, produces `STALE` within two sample intervals. Stop the throwaway runs afterwards. If the mechanism is
  unavailable or fails this, say so in the log, run the sampler as a plain background process, and rely on `check`.
- A shortage (an `ALERT` or `WORSE` line, or `check` reporting one) is an instruction to act: stop launching new
  sub-agents and work, remove jj workspaces, build output and scratch you own that are no longer needed, wait for or
  stop the job most likely responsible, and resume launches only once `check` passes. A monitoring failure (`STALE`,
  `ERROR`, a monitor exit, `check` failing on heartbeat age) pauses new launches until monitoring is restored; stop a
  hung sampler before starting another so two never write one status file.
- Run `watchdog check --status <file>` before every new build, test run, sub-agent or review launch, and at natural
  boundaries. Record the monitor handles, status file and watched paths in the log and carry them in every handoff; a
  resumed session whose status file is still fresh stops the earlier session's monitors (through the recorded handles,
  or the `pid` in the status file) before starting its own. Stop the monitors when the plan closes.

### PR discipline

- One reviewable commit, stable bookmark and draft PR per step, in a single linear stack, using the `jjstack` skill. The
  stack's base is `main@origin`, or this plan's own open PRs when it resumes, set up per `plans/.agents/executing.md`,
  "Executing one plan" (step 6); never another plan's PRs. Bookmarks:
  `plan/in-band-host-command-status/01-command-status-record`.
- Keep each PR bite-sized. Avoid churn: code added in one PR and deleted in a later one means the stack should have been
  shaped differently. Within this run, a PR that needs correcting is restructured rather than corrected on top, and that
  applies to PRs an earlier run built too.
- The last PR of the stack removes the TODO.md entry this plan covers ("Report remote command status in-band, not
  through ssh's exit status"), as root `AGENTS.md`, "TODO.md and FLAKES.md", requires. Leave the separate "Use the
  Tailscale SSH wrapper for Tailscale hosts" entry alone.
- Commit messages and PR titles use Conventional Commits, with the type reflecting the user-visible effect (root
  `AGENTS.md`), and a changelog fragment in the same commit when the type calls for one (root `AGENTS.md`, "Changelog
  fragments"). A later PR of this stack that amends work an earlier PR of it introduced edits that PR's fragment rather
  than adding a second one. This plan's `## Release note` section is the maintainer's answer for the release's
  highlights, not an instruction to you: each PR still leaves the fragment its type calls for, describing in user-facing
  words what was built, and saying in its context paragraphs where what was built departs from the release note
  (`plans/.agents/executing.md`, "Executing one plan", step 8). Never edit that section.
- Run every commit message, PR title and PR description through the `scode-commit-msg-reviewer` skill's cold read.
- `TODO.md`, `TRIAGE_OUTCOMES.md`, `TRIAGE_OUTCOMES_ARCHIVE.md`, `review_feedback_queue/` and `releasing/changelog.d/`
  are shared bookkeeping files: on a rebase conflict there, keep both sides.
- Before every push of a plan bookmark, run `queue status` and confirm the line still carries your claim id
  (`plans/.agents/executing.md`, "Executing one plan", step 9).
- Never edit `plans/`: the plan's state moves only through `scripts/plans-queue.py`, as `plans/AGENTS.md` describes.
- PRs stay drafts. Never mark one ready and never merge; landing is the monitor's job (`plans/AGENTS.md`).

### Review gate

Before finishing each PR, launch a review of its changes and address what it finds before moving on. The maintainer
demands exactly this reviewer, and no review swarm: a single fresh-context agent on `gpt-6.2-sol` at `high` effort,
launched through `$scode-harness-shellout` when your harness cannot serve that model natively. If it cannot be reached
when a review is due and is still unreachable after a reasonable retry, block the plan per `plans/.agents/executing.md`,
"Executing one plan" (step 10), after pushing what is consistent. Never substitute another reviewer.

The prompt carries this full charter, because the reviewer has nothing else:

> Review the changes in this PR adversarially for general correctness (bugs, unhandled cases, broken invariants, wrong
> behavior against the acceptance criteria quoted below), design (whether the shape of the change fits the codebase and
> the goal, and whether a simpler shape would), and idiomatic code for the languages involved. Report findings to the
> named file, most severe first, each with the file, the quoted code, what is wrong, and the smallest fix. Edit nothing
> and touch no VCS state.

The prompt must also name the repository root (the executing checkout), the bookmark or commit range of this one PR, a
findings file in the scratch directory, and the acceptance criteria (this file's goal, acceptance criteria, decisions
and outline). When a PR changes tests or their fixtures, include the full text of `.agents/test-authoring.md` verbatim
in the prompt, as root `AGENTS.md` requires.

The maintainer's standing rule for this reviewer (2026-11-17): its findings are inputs, not orders. Take a correctness
finding (a bug, a missed case, a wrong claim) after checking it yourself. A finding that would add code, a new
mechanism, a new test layer or another round of hardening is declined by default unless you judge it plainly necessary;
never let a review grow a PR. Log each finding you decline as a DECISION with its reason. Do not write a launch command
here or in the log; the harness-shellout skill owns launch mechanics.

### Scope reassessment

Run a fresh-context review at your own model and effort:

- before implementing a substantial departure from the outline above (the plan-specific examples are under Fallbacks
  above; they are examples, not a blacklist), including a new mechanism that does the job of an existing one or a
  generalization the outline did not plan;
- whenever the same component has needed repeated corrective review rounds.

Supply this file's goal and decisions, the outline, the current diff, and the proposed departure: what changed, why it
is necessary, and which simpler alternative was ruled out. Use this charter:

> Identify where this plan does more than the user needs, and where it duplicates a mechanism the repository already
> has. For each substantial new mechanism, explain which requirement makes it necessary and whether an existing facility
> or narrower supported behavior would suffice. Where the plan builds something that does substantially the same job as
> an existing mechanism, say whether that mechanism could be reused as it stands or generalized to serve both uses, or
> whether keeping them separate is better because sharing would blur two different contracts, and what generalizing
> would cost; never propose generalizing beyond the uses the plan actually has. Weigh the two against each other: a plan
> is not simpler for copying a mechanism, and not better for generalizing one nobody else needs. Propose the smallest
> coherent design that avoids needless duplication. Challenge planner-authored requirements; identify any simplification
> that would change an explicit user requirement or repository constraint, and mark any reuse-versus-duplication choice
> that is a close judgment call rather than clear-cut. Report concrete findings and alternatives, or say no unnecessary
> complexity or duplication was found. Do not manufacture objections, add safeguards to justify an unnecessary
> subsystem, or add abstraction to justify a generalization. Write findings to the supplied private file; change no
> product files or VCS state.

A close reuse-versus-duplication call that stays within the agreed scope is yours to decide after the reassessment, and
to log as a DECISION. Reuse an existing review checkpoint when it can answer the question; do not schedule periodic
reviews. A sub-agent gets a bounded unit within the outline; an unapproved new mechanism is outside its task, and one
discovered mid-unit is left unimplemented and reported back, with checks and incomplete work stated honestly. Apply the
same decision boundary as Unattended fallback below.

### Decision logging

Log each consequential choice in the working log as it happens, under a scannable `DECISION` label, with the
alternatives considered: in particular those named under Planner proposals and Fallbacks above, and every review finding
you decided not to follow. The maintainer will ask for these later.

### Unattended fallback

Resolve routine implementation forks within the agreed scope and log them as DECISIONs. Remove planner-invented
machinery that turns out to be unnecessary, as long as the maintainer's decisions and the repository constraints still
hold. Wording in this file that turns out inaccurate (a line number, a file name, a test name) is recorded in the
working log with the true value; never edit this plan file.

A material scope expansion, a weakened guarantee, or an omitted required behavior needs an agreed fallback or the
maintainer's decision; a review finding or a log entry is not authorization. If no fallback covers it, record the
concrete tradeoff and continue with independent work. Block per `plans/.agents/executing.md`, "Executing one plan"
(step 10) only for what cannot proceed without the maintainer, after pushing every PR that can be finished.

You run unattended, so never ask the maintainer anything in conversation: no question tool, no approval prompt, and no
"should I ...?" or offer of optional extras closing a turn, whatever a loaded skill or the harness suggests. Every fork
is either decided by you, taking the option that best fits this file, its Decisions and the specs, and logged as a
DECISION, or it is a block per `plans/AGENTS.md`. Things the maintainer should know and possible follow-ups go into the
report's "things you should know" and "open questions and possible follow-ups" sections, never into a question.

## Done criterion

The plan is complete when a linear stack of draft PRs exists, one per step in the order the outline gives (or as the
outline's fallbacks allow), together meeting the acceptance criteria above, each having passed its review gate, and the
last removing the TODO.md entry this plan covers. Open, not merged. If a `## Decisions` section exists, its latest entry
must also be satisfied. Then close the plan per `plans/.agents/executing.md`, "Executing one plan" (steps 11 and 12):
deliver its report through the queue script, write a closing entry in its log, and stop the watchdog. Never edit
`plans/` yourself.

## Release note

Adding remote hosts that are running tailscale ssh should now work, as well as other ssh servers that don't properly
respect and propagate the exit code of the program that got run. This does not mean we directly integrate with Tailscale
yet (Farhelm does not run "tailscale ssh"), but it means that regular ssh to a tailscale hostname should now work.

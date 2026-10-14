# Signed release advisories, starting with revocation

Written against main at 66af18cd on 2026-10-13. This is a goal file for an unattended run, executed through the planning
system in `plans/AGENTS.md`. You, the executing agent, have none of the conversation that produced it; everything you
need is here or in the repository files it names. SPEC.md and SPEC_impl.md stay authoritative over anything this file
says, except where this file's decisions direct you to change them.

This plan has no dependency on another plan. It touches the desktop updater (`crates/farhelm-ui/src/desktop/updater.rs`)
and its readout, as does the plan `update-check-readout.md`; the two may run at the same time, so expect a careful
rebase if that one lands first, and keep this plan's readout changes small and separate from its click-to-check button.

## The goal

An installed Farhelm learns about releases only through `get.farhelm.io/latest` (one unsigned line, parsed strictly)
and, while installing a release, that release's own signed `SHA256SUMS`. So nothing a later release needs to tell
installations already in the field can reach them: adding a line to `/latest` breaks every shipped app's update check,
and each `SHA256SUMS` is signed once, for its own version. Before the first release that needs to say something new,
ship a release that reads a new, signed, forward-compatible file, and that already understands the one directive that
matters most: "release vX.Y.Z is revoked".

What revocation does for the user, in the cases planning walked through with the maintainer:

1. **The running version is revoked.** Once any check (automatic, or on demand) has learned it, the version readout says
   this version was withdrawn, with the maintainer's one-line reason in the hover. With automatic updates off there are
   no background fetches (the maintainer kept SPEC.md's privacy promise), so the warning appears only after an on-demand
   check. The ordinary update path is unchanged (with automatic updates on, the fixed release installs as usual).
2. **A revoked release is installed and waiting for a restart.** Today the readout offers "Restart to update" into it,
   and the next ordinary relaunch opens it anyway, because the next launch runs whatever the last install put in place
   (the Installed record and `Contents/MacOS/farhelm-desktop`, which the same install replaced). Instead, the revoked
   waiting update stops being offered, and the updater installs a safe target over it so the next launch is safe. The
   target is the release `/latest` names, only when it is not revoked and `running ≤ target < waiting`; otherwise
   nothing is installed and the readout warns. A target below the running version is never installed: the running app's
   supervisor and helm have already migrated their databases, and older builds refuse a newer schema. A target equal to
   the running version is a reinstall of what runs, which the existing install path already handles.
3. **Never install a revoked release**: automatic and on-demand checks refuse a revoked target, and an on-demand check
   says why.

Acceptance criteria:

- A new file on the site, `/advisories` with `/advisories.minisig` (names are a planner proposal; choose and log),
  signed with the release key ring exactly as `SHA256SUMS` is (prehashed minisign, any key in `RELEASE_KEY_RING`), with
  a version-independent trusted comment (planner proposal: `farhelm advisories`). The desktop updater fetches it on
  every update check, automatic or on demand, after `/latest`, with small size caps, the same https-only, capped fetch
  path `/latest` uses, and a short timeout.
- Forward compatibility: the format is line-oriented with a fixed first line identifying the file. Lines a client does
  not understand are ignored; a known directive whose arguments do not parse is ignored with a log line. There is no
  format version an older client would refuse: new information is always new directives. The signature covers the whole
  file, so truncation and tampering are refused by verification, not by parsing.
- Only a fully verified file replaces the advisories the app holds in memory for this run (so a verified file that no
  longer lists a revocation lifts it). Anything else is "no new information" and changes nothing: a missing file (HTTP
  404) quietly, and every other failure (network error, timeout, other HTTP status, oversize, missing or bad signature,
  wrong trusted comment, bad first line, invalid UTF-8) with a log line. No failure ever blocks an update check or an
  install, panics, wedges the updater, or sets the "must be reinstalled" warning.
- While an update waits for a restart and automatic updates are on, the automatic schedule keeps running in a reduced
  form: it fetches `/latest` and the advisories file, and installs only when the waiting version is revoked (case 2).
  Today the automatic check is skipped entirely while an update waits (`automatic_due` requires that no update is
  ready), which would leave case 2 unreachable. Keep "the waiting update is revoked and no longer offered" separate from
  the gate that decides whether a check runs.
- Before installing a safe target over a revoked waiting update, skip the install (and warn) if the Running record names
  a version newer than the target: that is a supervisor started by hand through the `farhelm` command after the waiting
  version was installed, the one case in which the waiting version can have migrated state. Otherwise the waiting
  version cannot have touched user state: inside Farhelm sessions the `farhelm` command runs the session supervisor's
  version, only the owner of a state directory migrates it, and the helm's CLI paths do not migrate a busy helm. Confirm
  these premises in the code and record the evidence.
- The older install is performed by the target release's own published `install.sh`, not this repository's. Verify that
  the tagged installers of every stable release since v0.23.0 install over a newer installed version (planning read
  today's: it has no downgrade check, keeps the newer folder as the previous Installed version, and puts the main
  program, `Info.plist` and forwarder back to the target's), and that each accepts the installation's ownership record.
  Add both properties to SPEC_impl.md's permanent installer contract: installing over a newer version works, and the
  ownership record stays readable by older installers (a future record change would otherwise make every such install
  fail as a daily retried check). If an older tagged installer cannot do this, exclude those versions as targets (warn
  instead) and log the DECISION.
- The one directive this release understands: revoked, naming a stable release tag and a short reason. The reason is
  displayed as plain text, control characters stripped, length capped. Revoking a prerelease, a malformed tag, or the
  development build is ignored.
- Case 1, case 2 and case 3 above behave as described. The withdrawn-version warning reuses the readout's existing
  warning mark with its own hover wording, distinct from the "must be reinstalled" hover (a new look only if the two can
  show at once and would be ambiguous; log it). It stays while the running version is revoked and the app runs, and is
  relearned at the next check after a relaunch. With an update waiting that is not revoked, the readout stays the red
  update-ready readout and its hover also says the running version was withdrawn.
- SPEC.md ("Installation and updates") describes revocation as the user sees it, and its sentences "While an installed
  update waits for a restart the automatic check does nothing more" and "only an on-demand check replaces a waiting
  update" are amended for case 2; SPEC.md's privacy/outbound-connections paragraph names the new file (fetched with the
  same checks as `/latest`, nothing more with automatic updates off). SPEC_impl.md extends "Verification chain (D3)"'s
  permanent site contract with the new file, its format, its trusted comment and its failure rules; "The desktop app's
  updater" with the behavior, including its waiting-update rationale ("a third version replacing the folder of the one
  still running"), which the safe-target install must respect; and the installer contract as above.
  `releasing/AGENTS.md` (or a file it points to) documents how the maintainer writes, signs and publishes an advisories
  file, since the publishing tooling lives outside this repository and agents never run it.
- Out of scope, stated in SPEC.md/SPEC_impl.md as such: the helm (it fetches only its own version's files and keeps
  doing so), `scripts/install.sh` (unsigned by design), remote hosts, replay protection beyond what D3 already accepts
  (whoever controls the site can already withhold updates; revocation guards against the maintainer's own mistakes, not
  that attacker), and any directive other than revocation.
- Test coverage is held to a very high bar (maintainer's decision below), listed under Tests.
- Changelog fragment, `kind: added`. The TODO.md entry is removed.
- A linear stack of draft PRs, each passing its review gate. No PR is marked ready, and nothing is merged.

## Requirement sources

**The user's request (2026-10-13):** plan this Near term TODO entry. Its text:

> **Let releases publish new information without breaking older ones.** An installed Farhelm only learns about releases
> through the files it already reads on get.farhelm.io, so anything a later release needs to tell existing installations
> (that a release is revoked, say, and must not be installed or should be updated away from) can only reach them in a
> shape they already accept. Today the app reads `/latest` as exactly one line, so adding anything there fails every
> older app's update check. `SHA256SUMS` lookups go by name, so extra signed entries look harmless, but no installed
> release fetches anything beyond `/latest` and its own version's files. Decide where new signed information goes and
> make clients ignore what they do not understand there rather than refuse it. Ship a release that reads it, and test
> that a release keeps accepting what newer releases publish. This must ship before the first release that needs to say
> something new, the same way the upgrade epochs entry above handles new upgrade logic.

**The user's decisions (2026-10-13):**

- A new signed file, fetched with every update check, missing means nothing to say, unknown lines ignored.
- The first release understands revocation, and does "warn + replace a revoked wait": case 1's warning; case 2's
  withdrawal of a revoked waiting update and install of the newest non-revoked release over it; never install a revoked
  release. The helm is not included.
- After the planning review pointed out that "warn even with automatic updates off" would need background fetches the
  privacy paragraph rules out, the maintainer chose: with automatic updates off, nothing is fetched in the background,
  and a revocation shows only after an on-demand check.
- The planning review's narrower rules were adopted by the planner: the safe target must satisfy
  `running ≤ target < waiting` (never below the running version, which would break the already migrated local
  databases), and the reduced automatic schedule while an update waits.
- In the maintainer's words: "for the plan make sure to hold a very high bar on test coverage. this is almost never
  exercised. we don't want to find out some bug causes a crash or wedged state. the reading of revocation must have very
  defensive practices and really good test coverage of all failure modes - network i/o, bogus content, doesn't syntax
  check, doesn't parse, etc etc etc".
- Review gate: a single fresh-context gpt-6.1-sol reviewer at high effort, no swarm.
- No-workhorse mode, which `plans/AGENTS.md` requires.

**Binding repository constraints:**

- SPEC_impl.md "Verification chain (D3)" and "Release signing key" (the site layout is a permanent contract with every
  client; the key ring; trusted comments); "The desktop app's updater" (the strict `/latest` grammar, the permanent
  installer interface `FARHELM_INSTALL_SUMS_FILE` + `FARHELM_VERSION`, which this plan reuses for the older install).
- SPEC.md "Installation and updates", "Upgrade compatibility and client scale", and the downgrade/state-loss rule
  (agents surface missing downgrade paths and possible loss of data or state).
- `releasing/AGENTS.md`: agents never author or run the publishing tooling and never sign.
- Root `AGENTS.md`: Conventional Commits; Finishing work; Releases and the changelog; The live install is off-limits
  (never run or modify the maintainer's installed app or `get.farhelm.io`); Agent scratch space.
- Global instructions: tests never modify the test process's own environment variables; inject the fetcher, clock and
  key ring.
- `.agents/test-authoring.md` for every test change. `plans/AGENTS.md`, Executing one plan.

**Planner proposals** are the file names, the trusted comment, the line format, keeping the last good advisories in
memory only, and the outline below.

## Outline

Line numbers drift; find the code by name.

Here's what exists today:

- `crates/farhelm-ui/src/desktop/updater.rs`: `probe_latest_release()` and `latest_from_site()` (strict `/latest`),
  `fetch_capped` (https only, byte caps), `install_release()` / `run_installer` (fetches `/vX/SHA256SUMS`, `.minisig`,
  `install.sh`; verifies with `farhelm_helm::verify_signed_sums`; runs the installer with `FARHELM_INSTALL_SUMS_FILE`
  and `FARHELM_VERSION`), `check_and_install()` (compares the latest release against the Installed record; success is
  judged by the Installed record), `automatic_due()` (skipped while an update waits), `UpdaterState` (its
  `update_ready()` method and `needs_reinstall` field) and `readout()` in the same file, rendered by
  `crates/farhelm-ui/src/app_updater.rs` / `app_bar.rs`. Tests: the `make_rig()` rig with fake `Deps`; fixtures in
  `crates/farhelm-ui/tests/fixtures/updater-release/`. The real fetch function is untested today, its client is
  https-only, and it cannot tell a 404 from other statuses.
- `crates/farhelm-helm/src/provisioning/release_payloads.rs`: `RELEASE_KEY_RING`, `verify_sums()` (prehashed only,
  trusted comment must match), `parse_sums()` (strict). Fixtures in `crates/farhelm-helm/tests/fixtures/release/`; their
  READMEs say the test key's secret was deleted, so new signed fixtures need a new test key (generate one, commit only
  its public half in the test ring and the signed fixtures, record how in the fixture README).
- `scripts/install.sh` installs whatever release `FARHELM_VERSION` names and writes the Installed record
  (`Contents/Versions/installed`), with no downgrade check, so installing an older release over a newer waiting one is
  mechanically the existing install step; no new install code is needed, only changes to the newer-than comparison in
  `check_and_install()` and to `automatic_due()`. What matters is the published installers of the possible targets (see
  the acceptance criteria), not this repository's copy.

Here's the work, as a proposal:

1. **Verification and parsing**, as a small pure module: verify a signature over arbitrary bytes with the ring and a
   fixed trusted comment (generalize `verify_sums`'s core rather than duplicating it), then parse lines into known
   directives, ignoring the rest. Exhaustive unit tests here; sign fixtures only for the verification cases and feed
   content variants straight to the parser.
2. **Fetching and updater integration**: fetch after `/latest` on every check; hold the last verified advisories for
   this run; the reduced automatic schedule while an update waits; cases 1 to 3 in `check_and_install()` and the state
   the readout reads, with the safe-target rule and the Running-record guard.
3. **Readout**: the withdrawn-version hover wording on the existing warning mark.
4. **Spec, `releasing/` documentation, changelog, TODO removal**, and the verification of published installers.

### Tests (maintainer's bar)

Nothing here is exercised in normal life, so every failure mode gets a test that would fail if the defensive handling
broke. Test fetching at three levels, without building a TLS harness: every fetch outcome against the updater through an
injected fetcher; the capped read and status classification (including 404 versus other statuses) against a loopback
plain-HTTP server; and one check that the production client refuses an `http://` URL, which also covers a non-https
redirect. At least:

- Fetching: 404 (no new information, no log noise), other HTTP statuses, connection refused, DNS failure, timeout, a
  body over the cap, a body that ends early, a non-https URL or redirect, a signature file missing, empty, oversize or
  garbage.
- Verification: wrong key, a key not in the ring, legacy non-prehashed signature, wrong trusted comment (including a
  release's `farhelm vX.Y.Z` comment, so a release's signature cannot be replayed as advisories), signature over
  different bytes, file and signature swapped.
- Content: empty file, missing or wrong first line, CRLF, a trailing line without newline, invalid UTF-8, NUL and other
  control bytes, extremely long lines, many lines, unknown directives (kept as a permanent forward-compatibility fixture
  of "what a newer release might publish"), a known directive with missing, extra or malformed arguments, duplicate and
  conflicting revocations, revoking a prerelease, the development build, or a nonexistent version, reasons with control
  characters or over the cap.
- Behavior: case 1 learned by an automatic check, and with automatic updates off learned only by an on-demand check (no
  background fetch when off); case 2 with a target satisfying `running ≤ target < waiting` (installs it, next launch
  safe), with the target equal to the running version, with the target below the running version (installs nothing,
  warns), with `/latest` itself revoked (installs nothing, warns), and with the Running record naming a newer version
  (installs nothing, warns); the reduced automatic schedule while an update waits (fetches, installs only for a revoked
  wait); case 3 for automatic and on-demand checks; a revocation learned and then a 404 or a failed fetch (still revoked
  in this run); a revocation lifted by a later verified file; interaction with an update waiting that is not revoked;
  interaction with `needs_reinstall`; the safe-target install failing (no wedge, reason shown as a failed check, retried
  next check).
- Robustness: no panic on any input (consider a small property or table test over random bytes for the parser), and the
  updater's state machine returns to idle after every failure path.
- Forward compatibility: a test that loads the permanent "future" fixture and asserts it is accepted and its unknown
  lines ignored, documented as the contract that newer releases rely on.

### Validation

Follow root `AGENTS.md` "Finishing work", choosing the smallest set that covers the change:

- `cargo fmt --all -- --check`, `cargo clippy --all-targets -- -D warnings`;
- `cargo check -p farhelm-ui --features desktop` and, through the recorder, the desktop Rust targets
  (`cargo nextest run -p farhelm-ui --features desktop`, filtered while iterating, the package once at the end) and the
  farhelm-helm release-payload tests if `verify_sums` is generalized;
- `bash scripts/test-install-sh.sh` and `shellcheck` only if `scripts/install.sh` or its tests change (the published
  installers' behavior is verified by reading their tagged sources, `git show vX.Y.Z:scripts/install.sh`, and, if
  needed, running them against a fixture as `scripts/test-install-sh.sh` does);
- `python -B scripts/check-test-sleeps.py` per `docs/test-sleep-check.md`;
- `dprint check` on changed Markdown and `python3 releasing/check-changelog.py format`.

The report tells the maintainer exactly what to add to the external publishing tooling and that nothing is live until an
advisories file is published (absence is "nothing to say").

## How to run

### Files outside the repository

Per `plans/AGENTS.md` (Files outside the repository): this plan's working log is
`farhelm-plan-release-advisories-log.md` in the parent directory of the checkout you run in, derived as that section
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
  `plan/release-advisories/<nn>-<short-name>` (for example `plan/release-advisories/01-advisories-format`).
- Expected shape: a few PRs along the outline's steps, each leaving main correct on its own. Nothing user-visible should
  ship half-done: a reader that learns revocations but cannot act on them is acceptable as an intermediate PR only if it
  is inert. Avoid churn: code added in one PR and deleted in a later one means the stack should have been shaped
  differently. Within this run, a PR that needs correcting is restructured rather than corrected on top, and that
  applies to PRs an earlier run built too.
- Commit messages and PR titles use Conventional Commits; the user-facing PR is `feat`. Each `feat` PR adds its
  changelog fragment under `releasing/changelog.d/` in the same commit, per root `AGENTS.md` (Releases and the
  changelog). Validate with `python3 releasing/check-changelog.py format`.
- The last code PR removes the TODO.md entry this plan covers.
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
- the acceptance criteria: this file's goal, the user's decisions, the outline and the tests list.

When a PR changes tests, include the full text of `.agents/test-authoring.md` verbatim, as root `AGENTS.md` requires.
Ask the reviewer to check specifically: that no input or network outcome can panic, hang or wedge the updater; that a
signature for any other purpose cannot be accepted as advisories; that unknown content is ignored rather than refused;
and that every item in the tests list has a test that would fail if the handling broke.

Where you disagree with a finding, decide on the merits and log the DECISION. Do not write a launch command here or in
the log; the harness-shellout skill owns launch mechanics.

### Scope reassessment

Run a fresh-context review through galaxy-brain:

- before implementing a substantial departure from the outline above, for example persisting advisories across restarts,
  replay protection with stored sequence numbers, a helm-side fetch, changes to the installer, or a second directive
  (examples, not a blacklist);
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

- file names, trusted comment, line format and caps;
- how verification is shared with the helm's code;
- the evidence that a waiting-but-never-launched version cannot have migrated state (and the Running-record guard);
- which published installers can serve as safe targets;
- the readout's withdrawn-version look and wording;
- the stack's shape;
- every review finding you decided not to follow.

The user will ask for these later.

### Unattended fallback

Resolve routine implementation forks within the agreed scope and log them as DECISIONs. Remove planner-invented
machinery that turns out to be unnecessary, as long as the user's decisions and the repository constraints still hold.

A material scope expansion, a weakened guarantee, or an omitted required behavior needs an agreed fallback or the user's
decision; a review finding or a log entry is not authorization. Examples: adding the helm, changing `/latest` or
`SHA256SUMS`, dropping case 2, background fetches with automatic updates off, installing a target below the running
version, or an install over a waiting version that can lose state. If the code contradicts the premise that a
waiting-but-never-launched version cannot have migrated state (beyond the Running-record case), block with that as the
question after building everything that does not depend on it.

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

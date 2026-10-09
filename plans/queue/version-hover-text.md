# Plain hover text for the version readout

Written against main at adf78aba on 2026-10-08. This is a goal file for an unattended run, executed through the planning
system in `plans/AGENTS.md`. You, the executing agent, have none of the conversation that produced it; everything you
need is here or in the repository files it names. SPEC.md and SPEC_impl.md stay authoritative over anything this file
says, except where this plan's own PRs change them as described below.

This plan has no dependency on another plan.

## The goal

Hovering the version readout at the top of the sidebar shows jargon ("this client was built as farhelm 0.24.0") and,
during update activity, dense texts. Replace every hover text of the readout with plain language that says what the user
is looking at and, on the installed Mac app, what the readout does when an update is ready, without promising anything
the app does not do.

Acceptance criteria:

- Each state below shows the agreed text (D1 to D5), with `X`/`Y` the real versions:
  - installed desktop app, nothing pending: "This is Farhelm X. When a newer version has been installed, this turns red;
    select it then to restart into the new version."
  - web UI, and a desktop app that is not the installed release (no updater): "This is Farhelm X."
  - development build: "This is a development build of Farhelm, not a release."
  - helm reports a different version than the window: "The helm runs Farhelm Y; this window was built as Farhelm X."
  - development build facing a helm with a different version: "The helm runs Farhelm Y; this window is a development
    build of Farhelm."
  - every updater activity text (update ready, checking, installing, up to date, check failed, restart failed, and the
    needs-reinstall wrapper) rewritten in the same plain voice, meaning unchanged.
- The text selection is a pure function of the version, the helm's reported version and the updater state, with Rust
  unit tests covering every state above, including the release-build texts the browser tests cannot reach.
- The browser test that pins today's hover text expects the new text.
- A changelog fragment describes the change for users.
- The last code PR removes the TODO.md entry "Plain hover text for the version readout."
- A linear stack of draft PRs as in Outline, each code PR having passed the review gate. Not marked ready; not merged.

## Requirement sources

**The user's request (TODO.md, the maintainer's words):** "Plain hover text for the version readout. Hovering the
version at the top of the sidebar says "this client was built as farhelm 0.24.0", which is jargon. Say instead something
like: "This is version 0.24.0 of Farhelm. When Farhelm detects there is a newer version available, this will turn red
and you can click it to restart and upgrade." Check that this is actually true before using it: what the readout looks
like when an update is ready, whether clicking it is how the user restarts to update, and what the web UI, which has no
updater, and a desktop app that is not the installed release should say instead."

**What the planner found (at adf78aba; find code by name, line numbers drift).**

- The readout is rendered by `AppBar` in `crates/farhelm-ui/src/app_bar.rs`. Its hover (`data-tooltip`) is
  `version_tooltip`, which is the updater's readout tooltip when there is an updater and
  `idle_tooltip(skew::CLIENT_BUILD)` otherwise. `idle_tooltip` (`crates/farhelm-ui/src/app_updater.rs`) produces "this
  client was built as farhelm {version}". The updater's texts come from `readout()` and `activity_readout()` in
  `crates/farhelm-ui/src/desktop/updater.rs`; the needs-reinstall text wraps the activity text in parentheses
  mid-sentence, so that composition needs rewriting too, not only the leaf strings. When an update is ready, a
  `BarMenuToggle` with label "Farhelm {version}, update ready: {tooltip}" replaces the plain span.
- The proposed wording in the TODO is wrong in two places: the readout turns red (with an up-arrow) when a newer version
  is INSTALLED (`UpdaterState::update_ready()`: installed newer than running, whether the app's own background install
  or the installer run by hand), not when one is detected; and clicking opens a small menu with "restart to update" and
  "what's new", not a restart. With no update pending, clicking does nothing.
- No updater exists in the web UI (`AppUpdater` is an empty enum off `native_desktop`) or in a desktop app that is not
  the installed release (`desktop::updater::start()` registers nothing unless `active_bundle()` finds the app in
  `~/Applications/Farhelm.app` at a release version). Both show the same idle text today.
- When the helm reports a different version, the readout's visible text is the helm's version (`displayed_version`,
  `skew::HELM_BUILD_SKEW`) while the hover names the window's own build; today `version_tooltip` ignores skew. Skew can
  only arise in the web UI, since the desktop app runs its own helm.
- The development-build check that exists, `farhelm_helm::is_development_build`, lives in a crate the web build cannot
  use (`farhelm-helm` and `semver` are non-wasm dependencies of `farhelm-ui`), and the development text shows exactly on
  the web/no-updater path. A development build is version `0.0.0-unreleased`; `X.Y.Z-dev.N` dev releases are releases
  for this purpose.
- Tests pinning the texts: `the_readout_is_red_exactly_when_a_newer_version_is_installed`,
  `a_failed_restart_has_its_own_words` and `the_readout_hover_carries_a_user_started_check` in `desktop/updater.rs`; the
  browser test "the sidebar app bar shows the helm build and client tooltip" in `e2e/tests/sidebar.spec.ts`, which runs
  a development build against both a matching and a different helm version.
- SPEC.md "Installation and updates" stays true ("Its hover says which version is installed and that a restart finishes
  the update"). The docs page `website/src/content/docs/docs/using/update-and-uninstall.md` already describes the
  behavior correctly.

**The user's decisions (2026-10-08):**

- D1. Installed desktop app, nothing pending: "This is Farhelm X. When a newer version has been installed, this turns
  red; select it then to restart into the new version."
- D2. Web UI and a desktop app that is not the installed release: "This is Farhelm X." Nothing about updates.
- D3. Development build: "This is a development build of Farhelm, not a release."
- D4. Helm version differs: "The helm runs Farhelm Y; this window was built as Farhelm X." For a development build: "The
  helm runs Farhelm Y; this window is a development build of Farhelm."
- D5. Rewrite all the other updater hover texts in the same plain language, meaning unchanged.
- D6. Review gate: gpt-6.1-sol high, no swarm.
- No-workhorse mode, which `plans/AGENTS.md` requires for every plan.

**Binding repository constraints:** SPEC.md "Installation and updates" (the readout and its menu) and the hover text
rule ("Every clickable control and every icon that carries meaning has hover text"); SPEC_impl.md "The desktop app's
updater" and "Version and skew"; root `AGENTS.md` (Talking to the user, for wording; Finishing work; Conventional
Commits; Releases and the changelog; Sharing the machine with other agents; Agent scratch space; The live install is
off-limits), and `.agents/test-authoring.md` for test changes.

**Planner proposals** are the mechanisms in Outline that the decisions do not fix. Challenge them through Scope
reassessment rather than treating them as requirements.

## Outline

### PR 1: plain version hover texts

`feat:` with a changelog fragment (kind `changed`: hovering the version number now says plainly which version you are
running and, in the Mac app, what happens when an update is ready).

- One pure, wasm-reachable function chooses the hover text from the window's version, the helm's reported version (or
  none) and, where present, the updater's state. The development-build test is a check on the version string that both
  builds can run: either move the existing predicate somewhere `farhelm-ui` can use on every target, or compare against
  the one development version, whichever duplicates nothing; do not keep two copies of the rule.
- `AppBar` uses it for both the plain span and the update-ready toggle's label; keep the toggle label's structure
  ("Farhelm X, update ready: …") unless the new texts make part of it redundant.
- Rewrite the updater's activity texts and the needs-reinstall wrapper (D5) as whole sentences in the voice of D1. Keep
  the reinstall command in the text.
- Unit tests for every state in the acceptance criteria; update the three updater tests and the sidebar browser test.
- SPEC.md needs no change; if a sentence describing the idle hover helps, keep it to one. SPEC_impl.md: update any
  sentence that quotes the old text.
- Remove the TODO.md entry.

Out of scope: the readout's visible text, colors, menu and click behavior; the `?` menu's check for updates; the docs
website.

### Validation

Follow root `AGENTS.md` "Finishing work": `cargo fmt --all -- --check`, `cargo clippy --all-targets -- -D warnings`,
`cargo clippy -p farhelm --bins -- -D warnings`, `cargo check -p farhelm-ui --features desktop`, the `farhelm-ui` unit
tests through `scripts/record-test-run.py` (a narrow nextest selection), the desktop Rust targets selection (the updater
texts live behind the desktop feature), and `e2e/tests/sidebar.spec.ts` on Chromium and WebKit through the recorder
after building the web UI as root `AGENTS.md` describes. `python -B scripts/check-test-sleeps.py` (per
`docs/test-sleep-check.md`) since tests change. `python3 releasing/check-changelog.py format` for the fragment.

## How to run

### Files outside the repository

Per `plans/AGENTS.md` (Files outside the repository): this plan's working log is
`farhelm-plan-version-hover-text-log.md` in the parent directory of the checkout you run in, derived as that section
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
  `plan/version-hover-text/<nn>-<short-name>`.
- The stack follows Outline. Keep PRs bite-sized without churn: never add code in one PR that a later PR of this stack
  deletes. Within this run, if a PR needs correcting, restructure it rather than stacking a correction on top; that
  applies to all of this plan's own open PRs, including ones an earlier run built.
- Commit messages and PR titles use Conventional Commits, the type reflecting the user-visible effect. A `feat`, `fix`,
  `perf`, `style` or `revert` PR adds a changelog fragment under `releasing/changelog.d/` in the same commit, per root
  `AGENTS.md` (Releases and the changelog). Validate with `python3 releasing/check-changelog.py format`.
- Run every commit message, PR title and PR description through the `scode-commit-msg-reviewer` skill's cold read. Leave
  a PR description empty when the diff and title say everything.
- The last code PR removes the TODO.md entries this plan covers.
- Never edit `plans/`: the plan's state moves only through `scripts/plans-queue.py`, as `plans/AGENTS.md` describes.
- PRs stay drafts. Never mark one ready and never merge; landing is the monitor's job (`plans/AGENTS.md`).

### Review gate

Before finishing each PR that changes code, tests or scripts, use the active galaxy-brain skill to delegate a review of
that PR's changes, and address what it finds before moving on. The user demands exactly this reviewer, and no review
swarm: a fresh-context agent on gpt-6.1-sol at high effort, shelled out to the harness that serves that model when the
executing one cannot reach it natively. The prompt carries the full charter, because the reviewer has nothing else:

> Review the changes in this PR for general correctness (bugs, unhandled cases, broken invariants, wrong behavior
> against the acceptance criteria quoted below), design (whether the shape of the change fits the codebase and the goal,
> and whether a simpler shape would), and idiomatic code for the languages involved. Report findings to the named file,
> most severe first, each with the file, the quoted code, what is wrong, and the smallest fix. Edit nothing and touch no
> VCS state.

The prompt must also name the repository root (the executing checkout), the bookmark or commit range, a findings file in
the scratch directory, and the acceptance criteria for that PR: its part of Outline, the decisions that apply to it, and
the goal's acceptance criteria. For a PR that changes tests or fixtures, include the full text of
`.agents/test-authoring.md` verbatim, as root `AGENTS.md` requires. Where you disagree with a finding, decide on the
merits and log the DECISION. Do not write a launch command here or in the log; the harness-shellout skill owns launch
mechanics. A PR that changes only Markdown gets no review.

### Scope reassessment

Before implementing a substantial departure from the outline above (changing the readout's visible text, colors or menu,
a new updater state, or a second copy of the development-build rule; these are examples, not a blacklist), and whenever
the same component has needed repeated corrective review rounds, run a fresh-context review through galaxy-brain with
this charter, supplying the user's request and decisions above, this outline, the current diff and the proposed
departure (what changed, why it is necessary, and which simpler alternative was ruled out):

> Identify where this plan does more than the user needs. For each substantial new mechanism, explain which requirement
> makes it necessary and whether an existing facility or narrower supported behavior would suffice. Propose the smallest
> coherent design. Challenge planner-authored requirements; identify any simplification that would change an explicit
> user requirement or repository constraint. Report concrete findings and alternatives, or say no unnecessary complexity
> was found. Do not manufacture objections or add safeguards to justify an unnecessary subsystem. Write findings to the
> supplied private file; change no product files or VCS state.

Reuse an existing review checkpoint when it can answer the question; do not schedule periodic reviews.

### Decision logging

Log each consequential choice in the working log as it happens, under a scannable `DECISION` label, with the
alternatives considered: in particular where the development-build check lives so the web build can use it, how the
toggle label composes with the new texts, and the exact final wording of each updater activity text, and every review
finding you decided not to follow. The user will ask for these later.

### Unattended fallback

Resolve routine implementation forks within the agreed scope and log them. Remove planner-invented machinery that turns
out unnecessary when the decisions still hold. A material scope expansion, a weakened guarantee, or an omitted required
behavior needs an agreed fallback or the user's decision; a review finding or a log entry is not authorization. If the
work needs such a decision, record the concrete tradeoff and block per `plans/AGENTS.md` (Executing one plan, step 10).
Because the PRs form one linear stack, later PRs sit on top of a blocked one: finish the PRs before it, record the
question, and close the plan as blocked rather than building past it.

## Done criterion

The plan is complete when the draft PRs of Outline exist as one linear stack, together meet the acceptance criteria
above, and every PR that changes code, tests or scripts passed the review gate. Open, not merged. If a `## Decisions`
section exists, its latest entry must also be satisfied. Then close the plan per `plans/AGENTS.md` (Executing one plan,
steps 11 and 12): deliver its report through the queue script, write a closing entry in its log, and stop the watchdog.
Never edit `plans/` yourself.

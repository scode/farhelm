## What this was about

You asked for tmux and xterm.js to be removed from Farhelm completely, in four chained plans. This is the third. The
first ended the old private tmux server at a supervisor's startup and removed the running product's tmux. The second
took tmux out of setup, the desktop app, provisioning, the installer and the release payloads. This one removes what was
left in the test tooling, CI, the release brick test and the agent instructions. The fourth, `tmux-drop-from-docs`,
brings specs, docs and code comments in line and audits what remains. The only tmux that matters to Farhelm now is the
startup step that ends an older release's server, and a tmux the user runs inside a session.

Until now, the tests ran against a tmux built from checksummed source (with a zig toolchain pin), and the test recorder
(the script every test run goes through, which keeps each run's evidence) could refuse a run whose tmux was not that
exact build. That made sense while Farhelm's own terminals ran in tmux. After this stack:

- The pinned tmux build and its pins are gone. The few tests that still start a real tmux server use whatever tmux the
  machine has: the tests that build an earlier release's layout by hand and check that a starting supervisor ends it,
  the tests of the cleanup that ends a tmux server a killed test left behind, and the test of a user's own tmux under a
  session's scope (which, as before, needs the distribution's tmux at its standard place, `/usr/bin/tmux`). Without a
  tmux they print a `SKIPPED` line and return, like the tests that need a systemd user manager or SSH.
- A skipped test still counts as passed in the test runner's totals, so skipping would quietly lose that coverage. The
  release build's test gate and the Tensorlake sandbox template therefore install the distribution's tmux, and a deflake
  sweep (the tooling that runs every test suite once to find flaky tests) refuses to start without one.
- The test recorder's `--tmux` option is removed outright. A command that still passes it is refused before anything
  runs, instead of silently losing the check it asked for. The recorder's run summaries no longer report which tmux a
  run used, and read older records that still carry that field the same way as new ones.
- The desktop smoke and the browser stack's cleanup test no longer require or kill tmux, since nothing they start runs
  one, and the disabled browser CI job no longer installs it. A standalone script that probed tmux's control-mode
  protocol for Farhelm's old terminal code is deleted.
- The brick test no longer has a path for an old session running in tmux, or checks of each session's stored terminal
  kind. The previous stable, the baseline it upgrades from, runs its sessions in terminal hosts, so the old session must
  be in exactly one terminal host and Restart must move it to one new host. You had decided that the Rust tests cover
  the startup ending of old terminals; the brick test now says so.
- Agent instructions, the validation inventory, the release procedure files and the review queue's filters no longer
  describe tmux as part of how Farhelm or its tests work. The two review filters about Farhelm's own tmux (its tmux
  terminals, and a hung private tmux server) are gone; the hung-dependency filter keeps its systemd user manager half.
- The deflake driver changed with the recorder (it passed `--tmux` and built the pinned tmux), so its own end-to-end
  evaluation ran, twice, and was clean both times.
- In the test code, three scripts of the fake agent (the test stand-in for Claude Code and Codex) that only tmux-era
  tests used are removed, and the test-state crate's tmux pieces are documented in terms of the tests that remain.

## Things you should know

- **Ubuntu's tmux broke one test helper, now fixed.** The distribution's tmux on Ubuntu 24.04 is 3.4, older than the
  pinned 3.7c. It escapes control characters in its listing output, so the failure diagnostics that the tests' tmux
  guard collects (the guard is the helper that stops a test's tmux server when the test ends) parsed nothing, and one
  real-tmux test failed on it. The diagnostics now frame those listings with printable separators; the test-state tests
  pass on both 3.4 and 3.7c.
- **`.ci-tmux/` stays in `.gitignore`.** Your acceptance criterion said to remove it, but existing checkouts still hold
  the old build there (this executor's did), and jj tracks new files on its own, so without the entry a checkout's next
  snapshot would sweep a built tmux into a commit. Its comment says it is a leftover that is safe to delete. The second
  entry, `.ci-tmux-python/`, is gone: the old build script already deleted that directory on every run.
- **The brick test's Mac base image keeps Homebrew tmux.** The release under test needs none. But the releases the test
  installs first as upgrade baselines check for tmux at startup and refuse to start without 3.7c or newer, even though
  their sessions run in terminal hosts, because they still supported old terminals. The previous stable is such a
  release until one that needs no tmux takes its place, and the older bonus baselines always are. The Linux base's tmux
  recommendation is removed.
- **Bonus brick-test upgrades from releases before v0.34.0 cannot pass their continuity checks.** This predates this
  stack: those releases ran sessions in old terminals, so the remote command a bonus pass keeps running is in tmux, and
  the upgrade now ends it, as the first plan intended. The recipe now says so and tells the tester to report it as a
  known limit rather than a regression. Whether to stop asking for those checks there is an open question below.
- **Commands in historical records still pass `--tmux`.** Older FLAKES.md entries, plans and lore keep their commands as
  they were (frozen material). Rerunning one means dropping that option; `docs/test-run-evidence.md` says so.
- **Stored Tensorlake templates from before this change have no tmux.** They got it from the pinned build the harness
  pushed. The harness instructions say to rebuild such a template, and each Rust shard's own `summary.txt` (the sandbox
  report, not the recorder's) now names its tmux or says it is missing. None of this executor's templates are left.
- **Kept on purpose:** the test of the browser hunt tool (the script that repeats a spec to chase a flake) still asserts
  that the hunt passes no `--tmux` to the recorder, because that test replaces the recorder with a stand-in that would
  not notice a refused option. The brick test keeps one guard that refuses, with a clear message, a saved record of the
  old session from a run started under the earlier rules (which allowed no terminal host); the reviewer called it
  optional, and without it the comparison would still fail, just with a less useful error.
- **Not changed here:** comments in code and specs that still describe tmux-era behaviour (for example in
  `e2e/stack-slots.ts`, the browser specs and the fake agent), `docs/agent-screen-fixtures.md`'s reason for keeping
  tmux-captured screens, and the two investigation write-ups that mention the pinned tmux are left for
  `tmux-drop-from-docs`.
- The only Rust code changes are in test support (`farhelm-teststate`, `farhelm-fixtures`) and tests. Nothing in the
  product changes, so there is no changelog fragment.

## Open questions and possible follow-ups

- **Narrow the bonus brick-test continuity checks to baselines from v0.34.0 on?** Today a bonus upgrade from v0.23 to
  v0.33 is bound to fail those checks, for behaviour the product intends. Options: leave it as a documented known limit
  (what this stack does), or require the continuity workload only for baselines that ran sessions in terminal hosts, a
  small version condition in the harness. I recommend narrowing it, so bonus results stay meaningful.
- **Review queue items about the old tmux terminal code.** The removed filter used to keep findings about Farhelm's own
  tmux terminals out of triage; the open queue items about that code (35 queue files mention tmux, not all of them about
  it) are no longer filtered, but the code they describe is gone. Options: let the next triage remove them as covered by
  this removal, or remove them in a dedicated cleanup now. I recommend the next triage, which decides per item; one of
  them, `newline-initial-login-directory-transformation` (about how tmux rewrote a session's starting directory), cites
  the deleted pin file and build script as its evidence.
- **`plans/.agents/close-reset.md` still names `.ci-tmux/`** among the build caches a reset keeps. Plan PRs may not
  touch `plans/`, so this needs a planning or maintainer change. Options: a small planning PR now, or folding it into
  the next planning change that touches that file. I recommend the latter; it is harmless meanwhile.
- **When can `.ci-tmux/` leave `.gitignore`?** Options: remove it now (any checkout still holding the directory then
  risks committing it), at `tmux-drop-from-docs`' final audit, or later once you are satisfied no checkout holds one;
  deleting the directory in each checkout is all it takes. I recommend the final audit or later.
- **`scripts/test-nextest-cleanup.py` no longer runs.** It is a by-hand check of how the recorder lets nextest kill a
  stuck test. It builds a tiny one-crate workspace but inherits the repository's nextest settings, which first build the
  repository's test-fixture package, so it fails before running its test. This predates the stack (its only change here
  drops `--tmux none`), and nothing gates on it. Options: give that workspace its own nextest settings, or drop the
  check. I recommend the former, as a small separate fix.
- **Deflake instructions read as ambiguous in two places** to the evaluation's deliberately low-powered test agent:
  which checks a small fix for a failing test needs during a sweep, and whether `dprint check` reporting "no files found
  to format" counts as passing. Both predate this stack. Options: clarify `deflake/AGENTS.md`, or leave it. I recommend
  clarifying both with a sentence each.
- **The fake agent's `codex-record` script has had no caller for a while.** It is unrelated to tmux, so it was left
  alone. Options: delete it, or keep it for a future test. I recommend deleting it in an unrelated cleanup.

## PRs

- [#3377](https://github.com/scode/farhelm/pull/3377/changes) `test: use the machine's tmux instead of a pinned build`:
  the pinned build and pins, the recorder's `--tmux` mode, the release gate, CI, deflake and Tensorlake changes, the
  real-tmux tests' skip, the diagnostics fix, and every instruction that would otherwise be false after it.
- [#3378](https://github.com/scode/farhelm/pull/3378/changes) `test: drop the brick test's old-session-in-tmux path`.
- [#3379](https://github.com/scode/farhelm/pull/3379/changes)
  `chore: drop Farhelm's private tmux from instructions and test fixtures`: the remaining instruction and filter text,
  the unused fake-agent scripts and the test-state documentation.

## Checks run, reused and skipped

Reused: none; everything below ran on this stack.

Run locally, through the recorder, on the stack's revisions. The tmux on PATH was the distribution's 3.4 unless noted.

- The tests that start a real tmux (the test-state crate's 87, the three old-terminal tests, the guard's own test and
  the user's-tmux test): run `92f5a789`, 91 of 92 passed; the failure was the diagnostics problem above, fixed in #3377.
  Then the test-state crate alone on 3.4 (`ddce1477`, 87/87) and on 3.7c, a leftover copy of the old pinned build
  (`db56d60a`, 87/87). The review then had me remove a lookup unit test I had added (the crate's existing one covers
  it), so the crate has 86 tests: `83cefdc3`, 86/86. With no tmux on PATH, `ede4e5a1` and `12352b0b`: every real-tmux
  test printed its `SKIPPED` line. (`12352b0b` also had one unrelated failure, a test that runs `sleep`, which that
  deliberately stripped PATH did not contain.)
- #3379's crates: `9745b9fc` (166 tests, without the old-terminal ones) caught a bug in my first change to the guard's
  own test (needed because it had used the removed text method), 165 passed; after fixing it, `4be939bf` ran that test
  alone (passed), and the final `b52fdcbb` ran 169 tests (the test-state and fixture crates, the guard test, the session
  lifecycle tests and the old-terminal tests): all passed.
- The brick test's portable harness tests: `166a6ea6` and, after #3378's review fixes, `bdf3d4ad` (258 each).
- `scripts/test-start-stack-cleanup.sh` (`3c765eec`) and `scripts/desktop-smoke.sh` (`de10f32f`): both pass.
- `scripts/test-nextest-cleanup.py`: fails before its test for the older reason in the open questions (its first
  attempt, `d557cbc3`, was my wrong invocation).
- The deflake tooling evaluation, twice (before and after the review added the tmux check): both clean, with the
  expected flake, deterministic fix and finished sweep.
- Directly, as `.agents/validation.md` names them: the recorder, trace, summary, inventory, summarize, hunt, browser
  hunt, hunt-planner, capture-agent-screens and deflake Python suites; `cargo fmt`; `cargo clippy --all-targets` on the
  changed crates; `dprint check`; `python3 scripts/check-agents-md.py`; `dist generate --check`;
  `python3 releasing/rehearse-release-gate.py --self-test`; the Tensorlake harness's `deno test` and `deno check`;
  `shellcheck` on every changed shell script. All passed (the desktop smoke's remaining shellcheck notes all predate
  this stack).

On Tensorlake sandboxes, which you allowed for the tmux plans up to ten at once (at most five ran at once here), with a
template rebuilt from this stack's harness: the template's probe shard (`8bdbfc72`, 20 passed); the two browser specs
that use the changed fake agent, grid flood and mouse modes, on both engines (two shards, `87e97e82` and `f04de2ad`, 8
passed in total); and the real-tmux Rust tests (two shards, `36a559e0` and `4be7685e`, 91 passed in total, each shard's
report naming tmux 3.4). One of those 91, an old-terminal test, returned early on its sandbox with a `SKIPPED` line
because its systemd scope probe found no usable user manager there; it ran fully and passed locally. The shards run the
recorder in its plain-command mode, not its nextest or Playwright mode, as the harness always does, and the sandboxes
have Node 24.16 against 24.20 locally. Every sandbox and snapshot was removed afterwards.

Skipped: the full workspace and browser batteries, because no product code changed and the tests at risk (the ones that
start tmux, the tooling's own tests, the two specs using the changed fixture) ran directly; a GitHub rehearsal of the
release gate, because it starts a full hosted run and the workflow change is a package install plus removed steps,
checked by `dist generate --check` and the rehearsal's self-test; anything on a Mac, since nothing Mac-side changed
beyond brick-test documentation and its portable harness.

## Review gate

Each PR had the single fresh-context Opus 5.5 review at high effort the plan requires. None found a bug in product
behaviour; the findings were in the stack's own tooling and test changes, and all are settled below.

- #3377: eleven findings, all taken except the two kept by decision under "Things you should know" (`.ci-tmux/` and the
  hunt test's assertion). The main ones: a deflake sweep now refuses to start without a tmux (it used to build one); the
  Tensorlake instructions say old templates lack tmux and a shard's report names it; a recorder test that could pass
  without its fixture running now proves the fixture ran; the check that decides whether to skip now finds tmux exactly
  the way the tests then run it; one test now skips only the step that needs tmux.
- #3378: seven findings. Taken: the recipe's honest statement about pre-v0.34 bonus baselines (the narrowing itself is
  the open question above), a corrected claim about when the Mac base can drop tmux, a dead parameter, an overclaiming
  docstring, a test for a run whose new agent session is gone, two hard-to-read sentences. Kept by decision, as the
  reviewer allowed: the brick test guard described under "Things you should know".
- #3379: eight findings, all taken, including removing two more unused tmux-era fake-agent scripts, an unused method
  that rendered the guard's diagnostics as text (the separator fix stays), and the last "no tmux" contrasts in the
  validation inventory.

### Landing

#### What landed with it

Nothing else landed in this round. Since the stack's base, main gained only plan-queue changes and a trailing-review
record (#3380), which adds review-queue items and touches no file this stack changes. Because the stack changes the test
recorder, the release gate and instructions that everything else on main relies on, an independent reviewer checked what
on main still depends on what the stack removes: the recorder's `--tmux` option (now refused before any run starts), the
pinned tmux build and its pins, the old build directory `.ci-tmux/`, and the three removed fake-agent scripts. Outside
historical records, nothing still uses any of them. The release gate's steps run what they ran before minus the tmux
check, with the distribution's tmux installed where the real-tmux tests need it, and the generated release workflow
matches its source. Every test that starts a real tmux decides whether to skip by finding tmux exactly the way it then
runs it, and the copies of rules that the instructions keep word for word changed together. That review found nothing
that had to change before landing; its smaller points are the second and third fixes below, which I made myself and
checked as described under the checks.

#### Fixes made while landing

- **The real Podman environment test could not start on a clean machine, on main already** (fixed in #3377). The
  previous plan, `tmux-drop-from-install`, stopped Farhelm installing its own tmux build on remote hosts, so the script
  that stages this test's inputs now stages only the Farhelm release archive the test installs into its container. The
  test's prerequisite check still demanded the tmux build too and refused to start without it. The release gate runs
  this test on a fresh machine, so the next release's gate would have failed there. The check now asks for the archive
  alone; the scenario itself never used the tmux file, since provisioning no longer pushes one. The landing notes for
  `tmux-drop-from-install` reported this scenario passing; that run's input folder still held a tmux build staged before
  the change, so its prerequisite check passed on a stale file. The scenario's own coverage from that run stands.
- **SPEC_impl.md still listed the removed `binary` fake-agent script** (#3379), in the testing section's description of
  the stand-in agent the tests use, together with the raw-output coverage that script provided. This stack removed the
  script, so the sentence became false with this stack rather than merely dated, and `tmux-drop-from-docs`' final audit,
  which searches for "tmux", would not have found it. Both are gone from the sentence.
- **The run-all-tests instructions (`.agents/run-all-tests.md`) gave the wrong reason to need a tmux** (#3377): they
  said a full run without one would count the real-tmux tests as passes, but the deflake sweep that run uses now refuses
  to start without one. The reason is corrected; the requirement to have a tmux stays.

#### Things you should know that the report does not say

- **The release gate now tests the shutdown of an earlier release's tmux server against Ubuntu's tmux 3.4**, not the
  3.7c those servers actually ran. That step finds and ends the server by its socket, its command line and signals,
  never through tmux's own protocol, so the version should not matter, and you had chosen "whatever tmux the machine
  has".
- **The whole Rust workspace ran here**, which the executor had skipped; that is how the Podman problem above surfaced.
- **A landed plan still awaiting your review, `version-dblclick-flake`** (the version label's double-click flake),
  suggests "the pinned tmux setup" in its text. It matters only if you ask for a follow-up on it; an executor would then
  find that setup gone and use the machine's tmux.

#### Possible follow-ups

- The executor's open questions stand: narrowing the bonus brick-test continuity checks to baselines from v0.34.0 on,
  the review-queue items about the old tmux terminal code, `plans/.agents/close-reset.md` still naming `.ci-tmux/`, when
  `.ci-tmux/` can leave `.gitignore`, `scripts/test-nextest-cleanup.py` not running, the two ambiguous deflake
  instructions, and the unused `codex-record` fake-agent script.

#### Checks

The slow runs were on boat sandboxes at your request (2026-11-12: boats instead of Tensorlake), on four new 8-core
Ubuntu 24.04 machines. Each had the distribution's tmux 3.4 at its standard place, a running systemd user manager,
working self-ssh and Podman 4.9.3 with the pinned base image, all checked before any test ran. Boat's command runner
lets even an ordinary user read files whose permissions forbid it, which breaks tests that expect a refusal, so the runs
dropped that capability first. They used this stack's own recorder, with no `--tmux` and no pinned tmux anywhere, the
way the release gate runs after this change.

On the stack as delivered, restacked onto main:

- `cargo fmt --all -- --check`, `python -B scripts/check-test-sleeps.py`, `dprint check`,
  `python3 releasing/check-changelog.py format`, `python3 scripts/check-agents-md.py`, `dist generate --check`,
  `cargo clippy --all-targets -- -D warnings`, and `shellcheck` on the changed shell scripts (nothing at warning level;
  the informational notes in `scripts/desktop-smoke.sh` and `scripts/test-start-stack-cleanup.sh` are fewer than on
  main): passed.
- The tooling's own suites, run directly: the recorder (63 tests), run summaries, inventories, summarize, the nextest
  and trace modules, the Rust and browser hunts, the hunt planner, the agent-screen capture scrubber and the deflake
  driver; `python3 releasing/rehearse-release-gate.py --self-test`; the Tensorlake harness's `deno test` and
  `deno check`: passed. The portable brick-test harness through the recorder: 258 passed (`bc22d194`).
- The whole Rust workspace except the desktop app (`cargo nextest run --workspace --exclude farhelm-desktop`, from a
  nextest archive of the same tree, four slots per machine): 5,776 tests run, 5,774 passed (`b1c1e2a2`, `766246cd`,
  `3a83453e`, `cfe5fcba`). These runs did not keep a passing test's own output, so I could not read their `SKIPPED`
  lines; that the ten tests that start a real tmux server ran is inferred from their durations: the end-to-end ones took
  a third of a second or more and one test-state test nearly four seconds, the other test-state ones 15 to 45
  milliseconds, against about five milliseconds for an early return, so those shortest ones are the least certain. The
  executor's own run of the test-state crate on tmux 3.4 passed too (`ddce1477`, 87 of 87). One failure was a compile
  test of the test-support crate's macro, which builds a small crate offline from the development machine's Cargo
  download cache; boats lack that cache, so it fails on every boat run, and it passed locally (`1cc2fac4`). The other
  was the real Podman scenario's prerequisite check described under the fixes.

After the landing fixes, on the tree that merged:

- The real Podman environment scenario on a boat, with its inputs staged afresh and a newly built archive: passed in
  about two minutes, with no skip (`7fd7c3f6`).
- `cargo clippy --all-targets -- -D warnings`, `cargo fmt --all -- --check`, `dprint check` and
  `python3 scripts/check-agents-md.py`: passed.

Reused: the executor's desktop smoke (`de10f32f`), browser stack cleanup test (`3c765eec`), the two browser specs that
use the changed fake agent (grid flood and mouse modes, `87e97e82` and `f04de2ad`) and the deflake tooling evaluation.
They ran on the delivered stack. Since then main gained only plan-queue changes and the trailing-review record, and the
landing fixes changed only the Podman test's prerequisite check and two passages of documentation, none of which these
checks exercise, so their coverage still applies.

Skipped: the browser suite, because `e2e/` is unchanged apart from a benchmark note and the fake agent lost only scripts
no spec uses; a hosted rehearsal of the release gate, because the workflow diff only swaps the pinned build for a
package install and drops `--tmux` (checked by reading and by `dist generate --check`); the CentOS provisioning test,
whose script only lost `--tmux none` and a comment, with no `--tmux` left anywhere it calls; and anything macOS-only (no
Mac here).

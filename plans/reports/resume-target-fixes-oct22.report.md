# Restart resumes only the session's own conversation: three narrow fixes

## What this was about

On the 2026-10-22 triage page you chose three fixes so that **Restart** never resumes a conversation the user has left
or one that belongs to another agent running inside the session. Each had a limit of about 100 changed lines (code,
tests and docs), and over it the work was to come back to you; all three came in under it (91, 69 and 96 lines).

- **Restart with and the hooks opt-out.** **Restart with** restarts a session into a conversation you pick, and Farhelm
  keeps the previous conversation until the agent reports the new one. With Farhelm's conversation hooks turned off for
  that agent (`FARHELM_AGENT_HOOKS`) no report ever comes, so the old conversation stayed, and every later plain
  **Restart** reopened it. You chose "Clear the old conversation when hooks are off".
- **Grok under an unfamiliar program name.** Farhelm accepts a Grok conversation report only when exactly one Grok it
  recognises is running in the session, not counting the program the session started (usually a wrapper), and it
  recognises only two program names. If the session's own Grok ran under another name, a second Grok started inside it
  became the only one counted, and its conversation took over what **Restart** resumes. You chose to refuse the report
  when the program the session started itself looks like a Grok.
- **OMP started by running Bun on its entry file.** The same takeover could happen to an OMP session started by a custom
  command that runs Bun on OMP's entry file directly, because Farhelm treated it like a `bun x` package launch, whose
  starting program it does not check. You chose to treat such a launch like an installed `omp` command.

## Things you should know

- **Restart with, hooks off** ([#2636](https://github.com/scode/farhelm/pull/2636/changes)): the old conversation is now
  dropped in that case, through the same path Goose sessions already take, so a restart that fails before the agent
  starts still gets it back. After that, plain **Restart** stays unavailable, since no report will come; **Restart
  with** keeps working. Grok is not affected, because its reports come from callbacks you configure yourself, which the
  opt-out cannot turn off. SPEC.md ("Durability and resume") and SPEC_impl.md now say this.
- **Grok** ([#2639](https://github.com/scode/farhelm/pull/2639/changes)): when the program the session started looks
  like a Grok itself (its command name is `grok`, or its executable is called `grok-*`) without being one Farhelm
  recognises, the report is refused. A report from such a Grok was already refused before, so that Grok loses nothing.
  One case that used to work now stops: a wrapper named `grok` or `grok-*` around a recognised Grok. Farhelm now refuses
  that Grok's reports, so **Restart** cannot resume its conversation. The list of recognised programs is unchanged.
- **OMP** ([#2641](https://github.com/scode/farhelm/pull/2641/changes)): Farhelm now treats a direct `bun <entry>`
  launch like an installed `omp` command: if any other Bun or Node process is running at the top of the session, the
  report is refused. `bun x`, `bunx` and npm launches keep the old behavior, including `bun` with options before `x`
  (which the review caught). A session that is already running keeps the old treatment until it is launched again. If
  you roll back to an older Farhelm, a session launched this way loses **Restart** until it is launched again, because
  the older build does not know the new launch kind and refuses rather than guesses.

## Open questions and possible follow-ups

- The triage decision left one related case open on purpose, and it still is: when Farhelm cannot prepare the Pi or OMP
  reporter for a launch, a stale conversation can survive a **Restart with** the same way. That failure is only known
  after the restart has been committed, so it needs a different fix. Worth its own review finding if you want it closed.
- No test checks that a reporting agent with hooks on keeps its previous conversation across **Restart with** until the
  new one is reported. That gap predates this plan; a small supervisor test would close it.
- With hooks off for Claude, the session list still says Restart becomes available "shortly after start", which never
  happens without hooks. Also predates this plan; a candidate for a separate review finding.

## The PRs

- [#2636](https://github.com/scode/farhelm/pull/2636/changes): forget the old conversation on Restart with when hooks
  are off.
- [#2639](https://github.com/scode/farhelm/pull/2639/changes): refuse Grok reports under an unrecognised Grok in the
  pane; based on #2636.
- [#2641](https://github.com/scode/farhelm/pull/2641/changes): stop a direct bun OMP launch from crediting a nested OMP;
  based on #2639.

## Checks run, reused and skipped

All runs below were on each PR's final revision, through the recorder with the pinned nextest and tmux, four slots and
zero retries, and showed no runtime skips.

- #2636: recorded
  `cargo nextest run -p farhelm-supervisor --lib -E 'test(named_restart_clears_capture_when_hooks_silence_the_harness) | test(named_goose_restart_clears_live_capture) | test(agent_kind::)'`,
  run `983c9c1f-7795-44e2-b574-bcd72098b70e`: 82 passed. The first two are supervisor service tests that drive a real
  restart. The new Claude case was also run with the fix removed, run `ca788a28-da03-47c5-a7c8-e4649167a428`, and failed
  as it should.
- #2639: recorded `cargo nextest run -p farhelm-supervisor --lib -E 'test(procs::)'`, run
  `5c58cfa8-c21b-4ca3-90ac-e59a65b8e3f2`: 86 passed.
- #2641: recorded `cargo nextest run -p farhelm-supervisor --lib -E 'test(procs::) | test(agent_kind::)'`, run
  `61005dff-40e9-426d-afd9-fe27c23cc9b4`: 167 passed.
- At the tip: `cargo clippy --all-targets -- -D warnings` (whole workspace), `cargo fmt --all -- --check`,
  `dprint check` on every changed file, `python3 releasing/check-changelog.py format`,
  `python3 scripts/check-agents-md.py` and `python -B scripts/check-test-sleeps.py` (isolated interpreter) passed.
- Reused: none. Main moved during the run by plan-queue commits only; nothing touched these files, so no rebase was
  needed.
- Skipped: the end-to-end and browser suites, which these supervisor decisions do not reach beyond what the tests above
  cover. CI was not dispatched; it does not run on PRs.

## The review gate's outcome

Each PR got a fresh-context Opus 5.5 review at high effort with the plan, its triage entry and the test-authoring
checklist, and every commit message went through a fresh cold reader. The reviews' substantive catches, all fixed: #2636
needed the spec sentences and a corrected changelog entry, and its test was reshaped to share the existing Goose test's
body, which also covers the restore after a failed restart and kept the change inside its limit; #2639 needed its spec
sentence and a test case that really exercised the command-name half of the rule; #2641 would have refused real
`bun --silent x` package launches, and its new rule now sits with the other rules for each kind of launch, so a future
kind of launch cannot skip it. All three PRs cleared the review gate.

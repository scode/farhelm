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

### Landing

Landed as #2636, #2639 and #2641, merged bottom-up, with the content the report describes and two documentation fixes
described below. It was the third of three plans in this landing round, after tunnel-port-and-link-ranges and
triage-docs-oct22.

#### What landed since the stack was based, and whether it interacted

Since the stack's base, main gained sign-in-preservation-sweep (a browser that signs in again reloads the page, and a
failed GUI preference save is reported and discarded) and two TODO.md commits. A fresh-context reviewer and I read this
plan against them and against the other two plans in the round:

- The sweep changes only the GUI, and nothing in this plan's supervisor changes depends on it.
- triage-docs-oct22, landed just before this plan, adds SPEC.md text to the same "Durability and resume" section about
  Goose sub-agents and child Pi runs replacing the Resume target. It does not contradict this plan's text on Restart
  with, Grok or OMP.
- With Farhelm's hooks turned off for an agent, the supervisor really starts that agent without Farhelm's reporter
  (Claude and Codex get no hooks, Pi and OMP no extension), so forgetting the old conversation on a named Restart with
  is right for exactly those agents. Grok and agents Farhelm does not integrate are never affected by that switch.
- The new reading of a direct `bun` launch takes the first word that is not an option, the way Bun itself does, so
  `bun --silent x ...` is still treated as a package launch. The OMP docs page's description of direct launches stays
  accurate.
- Each of the three changelog entries fixes behavior that shipped in an earlier release, so each is its own entry.

#### Fixes made during the landing

- The "Stop, restart, and resume" docs page said Farhelm keeps the previous conversation after Restart with until the
  agent reports the new one, naming only Goose as the exception. With Farhelm's hooks turned off (`FARHELM_AGENT_HOOKS`)
  that is no longer true for any agent but Grok, so the landing added a sentence saying so, in #2636.
- #2639 now refuses Grok's conversation reports in a session whose resident wrapper is itself named `grok`, or whose
  wrapper program is named `grok-` followed by anything, so Restart becomes unavailable for such a session where it used
  to work. The report says so, but the changelog entry and the Grok docs page did not; the landing added the caveat to
  both, in #2639.

#### Checks

The round's three plans change the helm's link classification and localhost tunnel port, the supervisor's Restart with
and its checks of which Grok or OMP process reports a conversation, and specification text. This round ran the helm's
and the supervisor's own tests, the browser test for clicking links in a terminal, and the source checks, on all three
plans stacked together on this round's main (which already held sign-in-preservation-sweep). Everything below except the
last item ran before the round's documentation fixes (in tunnel-port-and-link-ranges and resume-target-fixes-oct22),
which change only three docs pages and one changelog fragment:

- `cargo fmt --all -- --check`, `cargo clippy --all-targets -- -D warnings`, the web-target UI lint
  `cargo clippy -p farhelm-ui --target wasm32-unknown-unknown --features web -- -D warnings`,
  `cargo clippy -p farhelm --bins -- -D warnings`, `cargo check -p farhelm-ui --features desktop`
  (tunnel-port-and-link-ranges removes a sentence from the desktop-only tunnel warning), the isolated test-sleep check,
  `dprint check` on the changed Markdown files, `python3 releasing/check-changelog.py format`,
  `python3 scripts/check-agents-md.py` (one warning, about a citation main already had), the docs website build, the
  workspace debug build and the UI's release web build: all passed.
- `cargo nextest run -p farhelm-helm` through the test recorder with pinned nextest and tmux, four slots, no retries,
  run `57e5a94a-29d0-4261-b9a5-697446134c41`: 1291 passed. This covers the link classification and tunnel tests
  tunnel-port-and-link-ranges added or changed, and the rest of the helm.
- `cargo nextest run -p farhelm-supervisor --lib`, same recorder settings, run `8604dc02-2297-49e6-a824-76be3cebf57d`:
  1258 passed, 4 skipped by design (helper entries that only run as child processes of other tests). This includes the
  Restart with tests that drive a real restart and every Grok and OMP process check.
- `npx playwright test tests/terminal-link-clicks.spec.ts` in `e2e/` on Chromium and WebKit, three workers each with its
  own helm and supervisors, no retries, run `6f350fd5-b9c0-42d3-a88a-f098d376cce2`: 30 passed.
- After the documentation fixes: the docs website build, `dprint check` on the changed Markdown files, the changelog
  format check and the instructions check passed again.
- Reused: none. Skipped: the rest of the Rust workspace (the farhelm binary's end-to-end tests and the UI tests; no code
  outside the helm and supervisor changed apart from removing one sentence from a dialog), the rest of the browser suite
  (only link classification reaches the browser), the doctests (no documentation example changed), the desktop smoke
  test, the native Mac app (not runnable here) and the installer (unchanged).

The report's checks ran the supervisor's Restart with, Grok and OMP tests on each PR; this round ran the supervisor's
whole library test suite on the plan as it landed. Nothing in the executor's report became untrue at landing.

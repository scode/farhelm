## What this was about

Seven terminal and session regression checks could accept broken behavior or reject valid output and fixture startup.
That weakened the evidence for Stop cleaning up an agent and its detached daemon, terminal replay preserving the
requested geometry, supervisor shutdown leaving the agent running, shell tabs returning complete results, and terminal
progress resuming after a stall. A hook fixture could also return before startup cleanup finished and then lose a new
launch specification. The maintainer chose small repairs to the named tests and local helpers, with product failures or
substantial expansion returned for a separate decision.

The orphan client is a terminal connection left behind after the supervisor is killed. The test checks that no leaked
stdin writer keeps it alive and that a replacement supervisor removes stale connections; a failed query or incomplete
inspection previously could look like proof that no writer remained. The hook fixture represents a supervisor ready to
accept new sessions. Its socket could exist before startup cleanup finished, so new launch data (the agent's startup
instructions) could be removed as stale before the test used it. A completed handshake now proves startup has finished
before that data is supplied.

All seven repairs fit that scope. Stop now exercises portable marker discovery with scope cleanup disabled; replay
checks wait for the complete agent echo; shell results have closing markers; progress must extend beyond replay; and
shutdown requires a fresh reply from the agent. The orphan-client check now refuses a failed client query or
inconclusive pipe inspection. The hook fixture waits for a real supervisor handshake after socket readiness. No
strengthened check exposed a product failure in the focused execution.

## Things you should know

This is test-only work. It strengthens regression evidence and fixture ordering; it does not change shipped behavior or
establish that users experienced the protected failures. Existing assertions remain. Diagnostic pipe-scan callers retain
their text reports, while the absence assertion additionally requires a successful inspection. A vanished client is
accepted only with a readable process-directory root and an absent client directory; missing stdin in an extant client
remains inconclusive. A matching holder disappearing during inspection is tolerated.

The shutdown probe shares a ten-second deadline across sending and capturing a unique agent echo, with a documented 25
ms polling interval. Hook readiness adds a twenty-second handshake bound while still observing serving-task failure. The
stream-progress baseline uses the highest complete replay record, falling back to the last pre-detach record only when
replay has none. No complexity gate tripped, no outcome was dropped, and no changelog fragment is required for this
`test:` PR.

The careful rebase inspected the installer refusal and UI license changes that landed since the claim, along with queue
and landing records. They do not change these fixtures, dependencies or observations. The rebase preserved the Rust
source diff byte for byte and kept both plans' bookkeeping. A later supervisor-test report changed only `plans/`; it
introduces no execution interaction.

## Open questions and possible follow-ups

No maintainer decision is needed to deliver this plan. Optional local mutation experiments were not run; the source
review assessed whether each observation distinguishes its named regression, and runtime validation exercised the
correct behavior. Native macOS execution remains unobserved. A separate plan owns other fixture-cleanup and test-oracle
findings; these seven repairs do not claim to settle them.

## The PRs

- [#1911 — tighten terminal and session regression checks](https://github.com/scode/farhelm/pull/1911/changes): one
  draft PR carrying all seven outcomes, their execution references, and removal of the corresponding feedback items.

## Checks run, reused and skipped

- Run: focused `cargo nextest run -p farhelm --test e2e -E <changed cases and fixture-consumer modules>` through the
  recorder, four slots and zero retries, with pinned nextest 0.9.143 and tmux 3.7c. Run
  `91c2229d-17b6-4cb9-99b2-1aa033de2794` passed 54 selected tests, including the new failed-roster query test, pipe-scan
  boundaries, changed terminal/session cases and hook, identity and resume fixture consumers. 317 tests were outside the
  selection. Retained output had no runtime `SKIPPED` and no omitted console bytes.
- Run after the final deadline amendment: the five supervisor-shutdown tests through the same recorder and substrate,
  run `e4932f56-ac14-4ec6-a422-51bc0f6af84e`; all five passed, with 366 outside selection and no runtime `SKIPPED`. The
  other 49 cases from the first run are reused because their code was unchanged. Each JUnit report additionally counts
  successful fixture setup; setup is not included in these test totals.
- Run: `cargo clippy -p farhelm --all-targets -- -D warnings`, `cargo fmt --all -- --check`, and the
  isolated-interpreter `python -B scripts/check-test-sleeps.py`; all passed, with 288 delays and zero missing
  rationales. Reused after rebase: the source and relevant dependency contracts were unchanged, so their coverage still
  applies. The documentation pass inspected all five touched Rust test files.
- Run again after PR URL bookkeeping: `dprint check TRIAGE_OUTCOMES.md review_feedback_queue/INDEX.md`; passed. The
  seven completed execution fields name the stable change, required bookmark and actual PR URL; unrelated entries
  remain.
- Skipped: full Rust battery, browser, desktop, installer, JS and doctest execution. This plan changes only the named
  Rust end-to-end tests and local helpers, and the focused run covers every changed scenario and fixture consumer. No
  additional integration risk justified those suites. Optional mutation experiments and macOS execution did not run.

## The review gate's outcome

The prescribed fresh-context gpt-6.1-sol/high source review returned no findings. It checked each strengthened
observation against its intended regression, the diagnostic wrappers, test-authoring contracts, documentation and scope.
The reviewer ran no runtime checks; the recorded runs above supply that evidence. The wording cold read accepted the
commit and PR text, and the native resume check reconciled the worktree, claim and retained evidence. The native report
cold read passed clarity and public-repository hygiene after adding product context for terminal cleanup and startup.

Private galaxy-brain evidence uses session UUID `a63f6b2c-45f7-4830-bf84-ea76de63472d`; its directory is recorded in
`farhelm-plan-e2e-session-test-fixes-log.md` beside the checkouts. The requested native model and effort are recorded,
but exact model attribution and usage were unavailable. Earlier estimated timestamps and one missing routing timestamp
are retained as evidence gaps; no cost or savings claim is made.

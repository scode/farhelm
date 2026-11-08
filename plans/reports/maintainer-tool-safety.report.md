# Maintainer tool safety

## What this was about

The terminal-driving tool could recursively delete unrelated work when `stop` received a directory that was not a
capture run. An empty argument could name the checkout itself. Separately, the screen-fixture scrubber and its
independent Rust privacy check could miss a local proxy URL containing a username or password before its host.

The maintainer chose two bounded fixes: leave unrecognized directories alone, and keep the scrubber's documented
replace-and-refuse promise. The stack makes `stop` fail without deleting a non-run directory, and recognizes userinfo
before localhost and bracketed IPv6 hosts in both URL guards. Each outcome's execution is recorded, and its feedback
file and index entry are removed in its own PR.

## Things you should know

The stop self-test covers an absolute non-run path, `.` and an empty argument, checking a canary's preserved bytes.
Normal start, input and real terminal-host shutdown checks still pass. The URL tests cover username-only and
username/password forms, with path, query-only and fragment-only suffixes. Public links with `@` in a path or query
remain allowed. The changes contain 24 and 76 added-plus-removed lines of code, tests and documentation, respectively,
within each 100-line gate; the agreed ledger, queue-removal and changelog exclusions apply.

No credential leak was observed. No vendor capture turns were spent, and captured screen files were not edited.

## Open questions and possible follow-ups

None required for this plan. The documented manual review of captured fixture diffs remains a backstop for personal data
outside the automated checks' patterns.

## The PRs

- [#3169](https://github.com/scode/farhelm/pull/3169/changes): refuse non-run stop paths and preserve their contents;
  draft head `19ad8b72`.
- [#3172](https://github.com/scode/farhelm/pull/3172/changes): scrub and refuse local fixture URLs with userinfo, with
  Python and Rust coverage; draft head `1d947057`, based on #3169.

## Checks run, reused and skipped

- Passed: recorder-backed `python3 -B scripts/agent-tui.py self-test`, run `e2d3aafe-4c6c-4c9c-9c48-bafe029b6b72`; the
  existing checkout-built Farhelm supplied the real terminal host.
- Passed: recorder-backed `python3 -B scripts/test-capture-agent-screens.py`, final run
  `ba86f5d6-43aa-4688-8561-fb83e39ffc82`, two tests covering 36 local URL cases and two public controls. Earlier run
  `0cf0f89d-31e6-445d-849e-f24efbd7b4e4` passed before query/fragment cases were added.
- Passed: recorder-backed `cargo nextest run -p farhelm-supervisor --lib` with filter
  `test(/^agent_kind::screen_fixtures::/) | test(/^agent_kind::status_line_fixtures::/)`, run
  `071fdbe4-480f-40e1-8cba-5005edbefe3a`: 10 passed, 1,784 selected out, pinned nextest 0.9.148, four slots, zero
  retries and `--tmux none`. Source capture, runner report and output are complete, with no runtime `SKIPPED` marker or
  cleanup limitation. The first attempt, `cb455126-f9c2-4344-8d19-addd3d4e51f9`, hit a five-minute command deadline
  during compilation, before any test ran; its retained evidence is incomplete as a test result. The final attempt
  allowed compilation to finish and kept the runner's per-test limits. This was no observed test assertion failure.
- Passed: `cargo fmt --all -- --check`, `cargo clippy -p farhelm-supervisor --all-targets -- -D warnings`, isolated
  `python -B scripts/check-test-sleeps.py` (423 delays, none missing a rationale), changed-Markdown `dprint check`,
  `python3 releasing/check-changelog.py format`, and `python3 scripts/check-agents-md.py` (zero errors, three existing
  citation warnings). An initial dprint wrapping error was corrected and the check passed.
- Reused: the clippy pass covers the final Rust logic; only two additional URL literals in its test changed afterwards.
  The stop self-test covered base `5b7172ca` plus its working diff; the final Python and Rust tests covered base
  `19ad8b72` plus the final URL working diff. Subsequent commit descriptions and ledger PR URLs do not affect those
  commands. Fresh main `f164d321` adds Credentials UI wording, its browser tests and specification, and plan/TODO
  bookkeeping; all upstream diffs were read and no interaction found. No rebase or runtime repetition was needed for
  those independent changes.
- Skipped: other Rust suites, browser, desktop, installer and provisioning tests. These maintainer-tool changes have
  focused script and fixture coverage; those suites add no identified coverage. No live installation or cloud check was
  needed.

## Review gate's outcome

Each PR received the required single fresh-context gpt-6.1-sol review at high effort. PR1 had no findings. PR2's
query-only and fragment-only URL gap was corrected, tested and accepted by the same reviewer. Separate cold readers
checked the commit and PR text. No finding remains unresolved.

### Landing

Landed as delivered, with no fixes at landing. The maintainer's terminal-driving tool no longer deletes a directory that
is not one of its runs when told to stop it, and the screen-fixture privacy checks now catch a local proxy URL with a
username or password before its host.

- #3169: the terminal tool's stop, its self-test, and its review feedback item's closing.
- #3172: the URL checks in the capture script and the supervisor's fixture test, the new test script
  (`scripts/test-capture-agent-screens.py`) and its entries in the check inventories, and the second review feedback
  item's closing.

#### What landed since the stack was based, and whether it interacted

Since this plan's base, main gained the Settings → Credentials fix (#3165), a TODO entry (#3167) and the Tensorlake
state-path refusal (#3168, #3170), and agent-settings-compare-replace landed first in this same round. None of those
touch the maintainer scripts, the screen fixtures or their checks; the plans share only bookkeeping files (the record of
triage outcomes, the review feedback queue's index and the changelog folder), where both sides were kept. A
fresh-context review found no interaction and nothing to fix. Both changelog entries are `kind: none`, as maintainer
tooling with nothing for users.

#### Things you should know

- The terminal-driving tool's stop (`scripts/agent-tui.py stop`) on a run that was already removed now fails instead of
  quietly succeeding; that is the intended result of refusing to remove what it does not recognize.

#### Checks

These ran once for both plans of this round, agent-settings-compare-replace and maintainer-tool-safety stacked in that
order on main `c0df0df4`, before the one landing fix (to agent-settings-compare-replace's changelog entry):

- `cargo fmt --all -- --check`, `cargo clippy --all-targets -- -D warnings`,
  `cargo clippy -p farhelm --bins -- -D warnings`, the check that every test delay states its reason
  (`python -B scripts/check-test-sleeps.py`, 423 delays, none without a reason), `dprint check`,
  `python3 releasing/check-changelog.py format`, `python3 scripts/check-agents-md.py` (no errors; three warnings about
  older text), `cargo build` and the UI's release web build (`dx build --package farhelm-ui --platform web --release`):
  all passed.
- Through the test recorder (`python3 scripts/record-test-run.py --kind development ...`):
  `python3 -B scripts/test-capture-agent-screens.py`, run `1694d374-58f2-491e-b1af-b5c32bdd2f50`, both tests passed;
  `python3 scripts/agent-tui.py self-test`, against the terminal host from this round's `cargo build`, run
  `43939a8e-3c8a-47a9-a26f-32798f7fe16b`, passed.
- The browser specs that drive credential delivery to hosts, `credential-delivery.spec.ts` and
  `tensorlake-hosts.spec.ts`, on Chromium and WebKit (`--runner playwright`, `npx playwright test` with those two files,
  three workers, no retries), run `12038435-72ac-4b5e-afa9-27273fe7c6c5`: all 28 passed.
- The workspace's Rust tests, all but the desktop app's own crate,
  `cargo nextest run --workspace --exclude farhelm-desktop` (`--runner nextest`, pinned nextest 0.9.148, the pinned tmux
  3.7c required, four slots, no retries, with rootless Podman and its offline image present), run
  `f9a26c13-0466-44cb-b5aa-a38c94f91b7d`: 5793 of 5794 passed. The 69 skipped are tests nextest leaves out by default
  (ignored tests that are only run by hand). The run keeps no output for passing tests, so whether any of them returned
  early with a runtime `SKIPPED` message was not checked. The one failure,
  `terminal_backpressure::shallow_pause_resumes_without_reset_or_replay` ("a pause lifted inside tmux's pause-after
  window must not trigger a catch-up reset"), is a recorded open flake: FLAKES.md's entries of 2026-09-03 and 2026-10-09
  record that same assertion failing under load, and it has a Deflake entry and is excluded from deflake sweeps. It
  tests terminal streaming code neither plan touches, and it was not re-run.
- After that fix, `python3 releasing/check-changelog.py format` passed; no code changed, so the runs above still apply.
- Reused: nothing from the executors' runs.
- Not run: the rest of the browser suite, the UI's JavaScript and desktop tests, the docs website build, a real
  Tensorlake sandbox and the native Mac app. No UI, website or browser-facing code changed beyond the helm's credential
  delivery, which the specs above drive; maintainer-tool-safety changes only maintainer scripts, a supervisor test and
  instruction files, which their own tests above cover.

Nothing in the executor's report became untrue at landing, beyond the PRs' draft state and head commits: the landing
rebased #3172 onto main after #3169 merged, so it is no longer based on #3169, and both are merged.

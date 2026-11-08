## Since the last review

The earlier execution blocked on the size allowance before changing code or opening a PR. The answered plan now has its
first implementation and draft PR, #3173; no earlier PR was changed or dropped.

## What this was about

Delivering recipe credentials to a Tensorlake host could silently undo a Claude Code or Codex settings edit saved while
Farhelm prepared the delivery. Triage chose to refuse replacing a file that changed after it was read, merge once with
the new contents, and leave the existing amber warning if another save conflicts again.

The final replacement now checks the old contents or expected absence. A second conflict preserves the saved file and
leaves delivery unrecorded, so the next connection tries again.

## Things you should know

The landing monitor raised the original approximate 100-line allowance to about 280 after the first execution found the
shared installer needed an old-content condition. The measured code, tests and specification change is 300 added plus
removed lines, near that approximate allowance. It reuses the uploader and existing warnings and delivery records; it
adds no lock, host protocol or retry task. Other installer callers retain unconditional installation.

The comparison and replacement run in the same finishing command on the host. A save made after the final comparison but
before replacement can still be overwritten; the check detects edits already present when that comparison runs.

## Open questions and possible follow-ups

An agent can still save its own stale settings after Farhelm successfully delivers credentials, undoing the new token.
That reverse direction was explicitly outside this outcome and remains unchanged.

## The PRs

- [#3173](https://github.com/scode/farhelm/pull/3173/changes): preserve concurrent agent settings edits; draft, based on
  main.

## Checks run, reused and skipped

- Passed: recorder-backed `cargo nextest run -p farhelm-helm --lib` selecting the credential-delivery module and
  `conditional_settings_install_preserves_a_concurrent_save`. Run `90be4abe-4080-4fbe-954a-387afa5f4bae`: 24 passed,
  1,544 selected out.
- Passed: the same command selecting existing local atomic convergence, remote setup ownership, upload failure cleanup,
  orphan cleanup and upload tampering tests. Run `d3e74cd4-ec14-45e5-9524-7db278f58ec3`: five passed, 1,563 selected
  out. Both runs used pinned nextest 0.9.148, four slots, zero retries and `--tmux none`; they use owned files and local
  shell stand-ins, not terminals or cloud services. Source capture, runner reports and retained output are complete,
  with no runtime `SKIPPED` marker or cleanup limitation.
- Passed: `cargo fmt --all -- --check`, `cargo clippy -p farhelm-helm --all-targets -- -D warnings`, the isolated
  `python -B scripts/check-test-sleeps.py` check (423 delays, zero missing rationales), targeted `dprint check`,
  `python3 releasing/check-changelog.py format`, and `python3 scripts/check-agents-md.py`. The last reports zero errors
  and three existing unrelated citation warnings.
- Reused for the final draft: these checks covered base `fcb7349b` plus the implementation working diff. The subsequent
  rebase onto `f164d321` has an empty interdiff; upstream changes were plan claims and a separate plan's report. Later
  edits only add the PR URL to the ledger. No executable, test or specification content changed.
- Skipped: other Rust suites, browser, desktop, installer and cloud runs. The selected tests cover the changed delivery
  contract and the shared installer's unchanged ownership, convergence and cleanup behavior; no broader coverage gap was
  identified. The installed Farhelm was not changed.

## Review gate's outcome

The required fresh-context gpt-6.1-sol reviewer at high effort found no actionable correctness, design or idiom issues.
The independent resume check and commit-message cold read passed. No finding remains unresolved.

### Landing

Landed with one changelog fix made at landing. When Farhelm delivers a recipe's credentials to a Tensorlake host, a
Claude Code or Codex settings edit saved after Farhelm read the file is no longer overwritten: the final replacement
checks the file is unchanged (or still absent), merges once with the new contents if not, and on a second conflict
leaves the file as saved, shows the existing amber warning and tries again on the next connection.

- #3173: the change, its tests, the SPEC_impl.md sentence, the changelog entry and the review feedback item's closing.

#### What landed since the stack was based, and whether it interacted

Since this plan's base, main gained the Tensorlake state-path refusal (#3168, #3170): Tensorlake connections now refuse
a helm state folder whose path contains `%`, `"` or `\`, which changes how the helm builds the SSH options for
Tensorlake hosts, in the same crate. The round also landed maintainer-tool-safety, which touches only maintainer
scripts, a supervisor test and instruction files. The landing monitor and, independently, a fresh-context review read
these against each other and found no interaction: the state-path refusal fails before any SSH command runs, as an
ordinary connection error, which the new code never mistakes for "the file changed", so it cannot trigger the new
merge-and-retry step. The shared file installer's other users (supervisor units, Codex login files, GitHub tokens) still
install unconditionally, as before. The review found the compare-and-replace logic sound: the check runs on the host in
the same command as the replacement, a file that appears where none was read (a dangling link included) counts as
changed, and a read failure is not retried as a conflict.

#### Fixes made at landing

- The changelog entry gains a paragraph for the release curator: a save landing between Farhelm's final check and the
  replacement can still be lost, since nothing locks the file, and an agent that saves its own older copy of the
  settings after delivery can still undo the delivered values (the report's follow-up). Per `AGENTS.md`, "Changelog
  fragments", such caveats go in the entry.

#### Things you should know

- The size: the plan's code, tests and specification come to 300 changed lines, against the "about 280" the landing
  monitor allowed when it answered this plan's block earlier on 2026-11-08 (the TODO entry to revisit that call landed
  in #3167). That is slightly over the number, within the scope the answer set: no lock, no protocol change, no
  background task, which the monitor judged acceptable rather than a reason to send the plan back.
- The review noted test gaps that are not defects: the tests use Claude's settings only, not Codex's (the same code
  handles both); the local-host case is tested with a file already changed rather than one changed mid-install; and
  nothing tests that an unreadable settings file is not retried. Failure messages of the installer's other users also
  changed wording slightly, with the exit status now inside the message; no code parses that text.

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

Nothing else in the executor's report became untrue at landing, beyond the PR's draft state, which the landing moved to
merged.

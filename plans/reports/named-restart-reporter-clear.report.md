## What this was about

Restart with could select a different Pi or OMP conversation while leaving ordinary Restart aimed at the previous
conversation. This happened when Farhelm could not prepare the extension that reports the agent's current conversation:
the launch continued without reporting, so nothing replaced the saved old target. The maintainer chose to treat this
like conversation hooks being turned off.

Farhelm now checks that extension before deciding whether to retain the old target. A preparation failure clears it
through the existing relaunch transaction. If the relaunch definitively aborts before starting the replacement, the old
target is restored; a successful or ambiguous launch keeps it cleared. The product and implementation specifications,
changelog fragment and triage bookkeeping agree with that behavior.

## Things you should know

Restart with still proceeds when extension preparation fails. Ordinary Restart remains unavailable until a later
accepted report establishes a conversation. The extension is checked again at ordinary launch time; the plan's accepted
same-account file-tampering window between those checks remains.

The tests use owned tmux sessions and a launch stand-in. They establish the supervisor's capture, rollback and refusal
behavior, including a valid old saved conversation, healthy-extension retention and a deliberately mismatched extension.
They do not launch real Pi or OMP agents or prove vendor reporting. No schema change was needed.

## Open questions and possible follow-ups

None required for this outcome.

## PRs

- [#2742](https://github.com/scode/farhelm/pull/2742/changes) — clear stale resume when reporter setup fails; one draft
  PR, head `c53c7c30889b13f8eb47132bd37751d5b34ef085`, not marked ready or merged by this executor.

## Checks run, reused and skipped

- `cargo nextest run -p farhelm-supervisor --lib -E 'test(/named_(pi|omp)_restart_clears_capture_when_reporter_preparation_fails/)'`
  passed both new regression tests in recorder run `052b4522-da72-4a5b-84f0-8a37bf8d134d`.
- `cargo nextest run -p farhelm-supervisor --lib -E 'test(/named_goose_restart_clears|named_restart_clears_capture_when_hooks|named_relaunch_clears_capture|pi_extension::tests/)'`
  passed all eight related tests in run `a6794385-abec-45e3-aa4e-5fa463e1d93a`.
- Both successful commands used nextest 0.9.148, pinned tmux 3.7c, four runner slots and zero retries. Their complete
  source evidence identifies base `6376dae9cbd3074506a51ca7dd725229d166c1f8` plus working-tree fingerprint
  `c95e281bd0acfc89437ea9df83dadc87792c6606ff089c5f03dbb1a8ab1f84ee`. Reports, output EOF and cleanup were complete. The
  1324 and 1318 skipped cases were outside the respective selections; neither run printed a runtime `SKIPPED` marker.
  These fixtures deliberately bypass systemd.
- `cargo nextest run -p farhelm-supervisor --lib -E 'test(/named_(goose|pi|omp)_restart_clears|named_restart_clears_capture_when_hooks|named_relaunch_clears_capture|pi_extension::tests/)'`,
  run `0d12ff1f-8de7-4e04-8683-fa4e792a9ede`, failed two of ten tests because their fixture used a bare conversation ID
  where Pi and OMP require typed locators. The corrected fixture establishes a valid exact record and OMP ownership
  provenance before acting. That failed command remains retained and is not reused as passing evidence; the two
  successful commands above cover its complete selection.
- `cargo clippy -p farhelm-supervisor --all-targets -- -D warnings` passed in recorder run
  `8e885fe8-8741-47c4-ba2e-ae0853f73e68`, with complete source evidence matching the runtime fingerprint above, full
  output and complete cleanup.
- `cargo fmt --all -- --check` passed for the final Rust source.
- `dprint check SPEC.md SPEC_impl.md TRIAGE_OUTCOMES.md review_feedback_queue/INDEX.md releasing/changelog.d/named-restart-reporter.md`
  passed. `dprint check TRIAGE_OUTCOMES.md` also passed on the published head after the PR-link amendment.
- `python3 releasing/check-changelog.py format` passed for the fragment.
- `python -B scripts/check-test-sleeps.py`, using the isolated interpreter prescribed by the repository, passed: 322
  calls inspected, no missing rationale.
- `python3 scripts/check-agents-md.py` passed with zero errors and two existing citation warnings unrelated to this
  outcome. It covers removal of the addressed feedback file.
- The successful commands above are reused individually for the published head. The patch was rebased from the tested
  base onto `660fb5b5`; the complete intervening main diff contained only another plan's delivery bookkeeping, and
  `jj interdiff` was empty. The static checks are reused on that same basis except the published-head ledger check
  explicitly noted above. Adding this PR's link to the execution ledger changed no executable input. The subsequent
  Steel UI landing changes neither the supervisor code nor the conversation-capture specification touched here, so no
  further runtime run or UI check was warranted.
- Browser, appshot, native desktop, installer and full-workspace runtime suites were skipped: this changes a bounded
  supervisor relaunch decision and its specifications, with no UI or installer change. The focused tests exercise the
  changed transaction and the existing asset contract.

## Review gate

The required gpt-6.1-sol/high reviewer found no issues in the implementation, specifications and final regression
fixture. The initial review missed the invalid fixture premise; the reviewer explicitly corrected that assessment after
the runtime failure and reviewed the strengthened fixture. The executor checked both findings artifacts against the
source. Commit and PR wording passed its fresh-context cold read.

### Landing

Landed as built. Restart with could start a different Pi or OMP conversation while plain Restart stayed aimed at the
previous one, when Farhelm could not prepare the extension that reports the agent's current conversation: the launch
went on without reporting, so nothing replaced the saved old target. As you chose, this is now treated like conversation
hooks being turned off: the old target is cleared, and restored only if the relaunch fails before starting. Restart with
still proceeds, and plain Restart stays unavailable until a later launch reports a conversation. SPEC.md and
SPEC_impl.md say so.

- #2742: clear the old Resume target when the reporting extension cannot be prepared.

#### What landed since the stack was based, and whether it interacted

Since the plan was based, main gained the Steel foundation (the interface restyle) and review-queue and triage records;
none of them touches restarting. restart-with-trust-identity in this round also changes Restart with, but only the
helm's decision whether to trust the folder; the two share no state. A fresh-context reviewer and I read all four plans
together and against what main gained, and found nothing that interacts.

#### Fixes made at landing

None.

#### Possible follow-ups

- The changelog draft says plain Restart stays unavailable "until the agent reports its conversation". When the
  extension could not be prepared, the running agent cannot report, so in practice it stays unavailable until a later
  launch reports. Worth a look when the changelog is curated.

#### Checks

This round landed four plans together, in this order: no overwrites during the Mac installer's command link placement
(installer-link-noreplace), folder identity for Restart with's automatic trust (restart-with-trust-identity), a link
refusal kept below an open session confirmation (link-refusal-confirm-layer) and clearing a stale Resume target when a
Pi or OMP reporter cannot be prepared (named-restart-reporter-clear). All four were built before the Steel foundation
landed earlier the same day. This round ran the following on all four stacked together on main with the Steel
foundation. The review queue's index and the triage record conflicted mechanically between these plans and entries main
had added; the resolutions keep main's entries, apply each plan's own update to its triage entry, and remove from the
index exactly the five findings these plans address (two for the installer plan, one for each of the others).

- `cargo fmt --all -- --check`, `cargo clippy --all-targets -- -D warnings`,
  `cargo clippy -p farhelm --bins -- -D warnings`, the check that every test delay states its reason, `dprint check`
  across the repository, `python3 releasing/check-changelog.py format`, `python3 scripts/check-agents-md.py` (two
  warnings, both about citations main already had), the workspace debug build and the UI's release web build: all
  passed.
- The tests of the UI's own JavaScript and stylesheet (`node --test` in `crates/farhelm-ui/js-tests`), run
  `ff182543-0d7a-46fb-a5f8-3b3c6214ad44`: 358 passed.
- The terminal link-click browser spec, `npx playwright test tests/terminal-link-clicks.spec.ts` in `e2e/`, on Chromium
  and WebKit, three workers each with its own helm and supervisors, no retries, run
  `eff1253f-b467-4151-bf3b-9f5081e7c1b3`: all 34 passed, including the new tests that keep a Delete confirmation's
  warning above a link refusal in both opening orders, now with the Steel restyle's buttons and fonts.
- The whole Rust workspace, `cargo nextest run --workspace --exclude farhelm-desktop`, with the pinned test runner and
  tmux, four tests at a time, no retries, run `8433a77d-dde8-4a20-95c7-b1bd78c1f791`: 4455 passed, 40 skipped by design
  (helper entries that only run as child processes of other tests, and tests that need a real signed-in Claude or
  Codex), and 1 failed: the terminal backpressure test "shallow pause resumes without reset or replay", a known flake
  with three entries in the flake log (the latest records it failing 2 of 3 times when run alone). Run alone on the same
  code three times, it passed each time (runs `a3b3ea0f-39c8-45ee-be09-e64b81ed92a3`,
  `76989247-159f-4240-9c5b-325259d1109c` and `b525f8e0-98e9-41b4-a217-3984ca31bf47`). No plan in this round touches the
  terminal's flow control. The deflake sweep skips this test on behalf of a TODO.md Deflake entry that does not exist;
  either add the entry or drop the skip.
- The Mac installer fixture, `bash scripts/test-install-sh.sh --farhelm target/debug/farhelm` after
  `cargo build -p farhelm` (it runs the installer against fake releases on Linux, standing in for macOS), run
  `a4f7acf6-7f4f-490e-b15f-e76c5143a4cf`: all 603 checks passed.
- Reused: none. Skipped: the full browser suite (only one plan changes the UI, and its spec file ran on both engines),
  the desktop app checks (no desktop code changed), real Pi, OMP, Claude and Codex runs (the reports say what their
  tests stand in for), and a real Mac installation (not runnable here).

The report's checks were the supervisor's restart tests; this round ran the whole Rust workspace with the other three
plans. Nothing in the executor's report became untrue at landing.

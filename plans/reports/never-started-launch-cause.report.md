## What this was about

A launch could fail before the agent existed, either in the user's login shell or in the systemd scope wrapper that
starts it on Linux. The scoped error named both suspects; hosts without scopes showed an ordinary exit. The shell's or
wrapper's own explanation also disappeared with the terminal, even though the session's error survived.

You chose to distinguish the failed step, show the exit code when known, and keep a bounded permanent copy of useful
startup output with the error. You also authorized rewriting the terminal-retention promise around useful exceptions
such as error reporting, while preserving the separate rule against saving or painting a full-screen program's frame.

## Things you should know

New scoped launches distinguish a login shell that never reached Farhelm's command from a scope wrapper that failed
before starting the agent. Older scoped launches keep the ambiguous explanation because they recorded no evidence that
separates those steps. Unscoped launches, including older ones, now report the shell failure as error. A missing exit
code is omitted rather than invented. User Stop and reboot interruption keep their existing meanings.

The durable error may keep the failed launch's last five nonblank terminal lines, at most 500 UTF-8 bytes after unsafe
characters are escaped. Printable separators keep those lines readable in the existing badge tooltip. Saved quotes
survive removal of the terminal. This adds no conversation history store, restart snapshot, or new UI surface.

A quote is omitted when its launch cannot be proved. Restart reuses the session's terminal, including earlier
scrollback; resizing that terminal after the new launch makes its startup quote unavailable. Any terminal also loses
quote eligibility when resized after its process has died. A fresh terminal resized while alive can still supply its
later startup failure. If the supervisor restarts before classifying a startup failure, it loses only the optional quote
evidence: the saved launch evidence still distinguishes shell from scope wrapper. An already saved quote remains. A
failed terminal capture disables further quotes for that observer pass, while the errors themselves still persist. The
TODO request to identify the failed startup step and retain its useful output is removed.

## Open questions and possible follow-ups

No maintainer decision remains. The optional separate check for a vanished Farhelm executable was omitted; a failure
before the first Farhelm process keeps the shell-stage wording and whatever terminal explanation can safely be kept.

## The PRs

- [#2493](https://github.com/scode/farhelm/pull/2493/changes): distinguish shell and scope-wrapper startup failures and
  classify the unscoped case as error.
- [#2499](https://github.com/scode/farhelm/pull/2499/changes): retain a fenced, bounded terminal explanation with the
  durable error and update the retention promise.

## Checks run, reused and skipped

All runtime checks use `python3 scripts/record-test-run.py --runner nextest`, nextest 0.9.148, the checksummed tmux 3.7c
build, four slots and zero retries. Recorded skips from test selection are separate from executed tests.

- `cargo nextest run -p farhelm-supervisor --lib -E 'test(launch_excerpt_)'`: all 10 focused excerpt tests passed in
  `8211d2ac-9c83-4573-8ae4-1968250ae96e`, including decimal-boundary guards, live/dead resize, later generations, reused
  scrollback, one-row terminals, literal ownership and sanitizing bounds.
- `cargo nextest run -p farhelm-supervisor --lib -E 'test(launch::tests::) | test(service::launch_artifacts::tests::) | test(service::status::tests::) | test(tmux::tests::) | test(one_excerpt_timeout_disables) | test(a_renamed_entry_shares_the_runs_cells) | test(a_relaunched_entry_gets_fresh_cells) | test(reload_rediscovers_the_pane_of_a_launching_row)'`:
  all 149 selected tests passed in `179a4832-dcb1-4221-8ae6-6a8c5c3a7488`. This covers the shared streaming runner,
  resize paths, classification, artifact cleanup and generation isolation.
- `cargo nextest run -p farhelm --test e2e -E 'test(an_unscoped_login_shell_startup_failure) | test(stopping_during_login_shell_startup) | test(a_failed_scope_wrapper_classifies) | test(without_a_user_manager_a_launch_records) | test(stop_is_idempotent)'`:
  five tests passed in `47928f28-db11-4d7f-8875-c8abcaeb2073`, plus the fixture-build setup counted separately by JUnit.
  The real Linux systemd scope path ran; there were no runtime `SKIPPED` messages. These checks include the saved quote
  after terminal removal, the no-manager fallback, and Stop while shell startup is still live.
- `cargo clippy --all-targets -- -D warnings` and `cargo clippy -p farhelm --bins -- -D warnings` both passed on the
  final rebased tree, covering the test-enabled and shipped configurations separately. `git diff --check` passed.
- `python -B scripts/check-test-sleeps.py`, using the isolated pinned parser environment, passed with 309 delays and
  none unexplained. Its first attempt exceeded its time budget and was incomplete.
  `dprint check SPEC.md SPEC_impl.md TODO.md` and `python3 releasing/check-changelog.py format` passed on the rebased
  tree; the latter validated all 70 local fragments. `cargo fmt --all -- --check` also passed on that tree. The
  delay-check result is reused because the rebase left this plan's test source and delays unchanged.

The final 149-test and lifecycle commands captured base `91c2853d` plus working-source fingerprint
`ee3904a706f171020eb81d123b1309c414ca9c70480a6dd21b12c5adf9895f4b`, later snapshotted as `f72499de`. Their source
identities, JUnit reports and retained output are complete, with normal cleanup and no omitted output. They are reused
after the rebase onto `04d52765`: both PR interdiffs are empty, and all nine changed Rust files remain byte-identical.
The landed UI peer-text change preserves the existing error tooltip's presentation of the excerpt; the later
uninstall-refusal changes affect separate helm operations and SPEC topology prose. No behavior correction or new runtime
coverage was needed for either interaction. The earlier 10-test excerpt selection ran before only a changelog wording
edit; its tested Rust source is also unchanged.

Retained failed attempts remain part of the evidence. `c4da25e4-f98a-4b3b-961e-2605fd71b9cc` failed compilation before
executing tests; the Stop exit-code lookup was corrected. `96510b31-b869-45d2-bc5b-3dbd6bb0be2c` ran 145 tests with two
failures in newly written test fixtures: an incorrect last-five-lines expectation and a session name changed by tmux's
format expansion. Both fixtures were corrected before the passing focused and complete selections above. These were
same-session development mistakes, not recorded latent flakes.

Browser, desktop, installer and provisioning tests were skipped because no UI, wire schema or installation behavior
changed. The existing error-tooltip path was checked against the landed UI text changes. No full workspace battery was
needed beyond the targeted launch and terminal coverage.

## Review gate outcome

Each PR has one independent gpt-6.1-sol/high code reviewer. PR1 passed. PR2's numeric-comparison and
dead-terminal-resize findings were corrected and passed the same reviewer's source recheck; the focused runtime gates
then passed. Commit and PR wording passed their separate cold reads. The capture scope reassessment selected native
terminal operations over additional shared geometry state, and the upstream interaction audit found no conflict with the
landed UI text changes.

### Landing

Landed as #2493 and #2499, merged bottom-up, with the same content the report describes, rebased onto this round's main,
with one conflict resolution in the supervisor described below. It was the third of five plans in this landing round,
after ui-session-actions and connection-stall-phase; yolo-confirm-launch-guarded and host-changed-update-warning
followed it.

#### What landed since the stack was based, and whether it interacted

Since the stack's base, main gained helm-ingress-types (typed request parsing in the helm, and tighter rules on how the
helm reports errors that came from a supervisor) and create-reservation-retention (a create's retry key is freed when
its session is deleted). A fresh-context reviewer and I read them, and the other four plans of this round, against this
plan.

- create-reservation-retention reworked how a failed or deleted create releases its retry key in the same supervisor
  code where this plan removes its new saved launch evidence (the file recording which startup step a launch reached).
  The two do not depend on each other: that evidence is removed alongside the launch's other files, and the retry key is
  still released exactly as main now does it.
- helm-ingress-types changes no code this plan touches; this plan changes only the supervisor and its own launch helper.
  The workspace run below includes this plan's end-to-end launch-failure tests.
- connection-stall-phase, earlier in this round, adds connection detail to unavailable session cards. That text comes
  from the host's last connection error, while this plan's text comes from the session's own launch error, so neither
  replaces the other. Both plans also change the `farhelm` command's entry point (that plan for the remote helper's
  progress line, this one for its launch step); both changes are present and independent.
- ui-session-actions, yolo-confirm-launch-guarded and host-changed-update-warning do not touch launch startup or its
  error.

#### Fixes made during the landing

- In the supervisor's cleanup after a create whose session was deleted while it was still launching, this plan's removal
  of the saved launch evidence conflicted textually with a comment create-reservation-retention added about the retry
  key. The landing kept both: the launch evidence is removed with the launch's other files, then the comment and the
  unchanged return follow.
- Its TODO.md removal conflicted with an entry main had removed in the meantime; the landing kept both removals.

#### Checks

Together the five plans change the session launcher and Replace in the UI, the helm's connection attempts and host
updates, and the supervisor's launch startup and terminal handling, about 8,900 added and 2,200 removed lines, including
changes to five browser spec files; that reach is why this round ran the whole Rust workspace and the full browser
suite. All of the following ran on the five plans stacked together on this round's main, including the landing's
conflict resolutions:

- `cargo fmt --all -- --check`, `cargo clippy --all-targets -- -D warnings`,
  `cargo clippy -p farhelm --bins -- -D warnings`, `cargo check -p farhelm-ui --features desktop`, the isolated
  test-sleep check, `dprint check` on every changed Markdown, JSON and TOML file,
  `python3 releasing/check-changelog.py format`, `python3 scripts/check-agents-md.py`, the website build (a docs page
  changed), the debug build, the release web build of the UI and `scripts/check-desktop-assets.sh` (which also builds
  the desktop app): all passed.
- `cargo nextest run --workspace --exclude farhelm-desktop` through the test recorder with pinned nextest and tmux, four
  parallel test slots and no retries, run `3907499f-e2bc-4ba3-9a83-ce6526597d54`: 4078 passed, none failed; 40 tests
  were left out by the suite's own filters. Its retained output is complete and has no runtime `SKIPPED` message, so the
  real systemd-scope and SSH tests these plans rely on ran.
- `cargo nextest run -p farhelm-ui --features desktop`, run `63f71d93-80d2-464c-841e-4d7823a61c33`: 773 passed.
  `node --test` in `crates/farhelm-ui/js-tests`, run `e320936a-a430-48b7-b45b-e8a5b2ef6f57`: 328 passed.
- The full browser suite (`npx playwright test` in `e2e/`) on Chromium and WebKit, three workers each with its own helm
  and supervisors, no retries, run `3157847c-bee9-4df2-b327-1137c87e135f`: 2270 passed, 14 skipped by design (WebKit
  clipboard cases and tests that need a real signed-in agent or run on one engine only), none failed or flaky; the only
  runtime `SKIPPED` messages are those WebKit clipboard cases.
- Skipped: the workspace and desktop doctests (no documentation example changed), the desktop smoke test (the desktop
  app crate is unchanged; the UI's desktop feature is covered by the check and tests above), the native Mac app (not
  runnable here) and the installer (unchanged).

The report's checks ran on a main from before helm-ingress-types and create-reservation-retention landed, and it skipped
the full Rust runtime suite and the browser suite; both ran this round, above. Nothing in the executor's report became
untrue at landing.

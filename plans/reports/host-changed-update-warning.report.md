# Host update continuity and warnings

## What this was about

Updating a host added while it ran an incompatible Farhelm could finish every step and reconnect successfully, yet leave
a yellow "host changed" warning. The incompatible protocol had prevented the helm from recording the host's identity;
the update's reconnect recorded it for the first time, which the UI mistook for a different installation.

The update now remains followed through that first identity discovery, including when its acceptance reply is still in
flight. The first learned identity becomes the comparison point, so a later replacement or disappearance still detaches
the update. A helm-side regression confirms that an update of a protocol-skewed host records its identity.

The maintainer chose to rewrite only the two changed-host warnings, include old and new values, and clear the warning
once the host connects at this helm's version. Real ssh destination or identity changes now name both values, explain
whether Farhelm lost sight of an accepted update or cannot tell whether submission started, and advise checking the
displayed farhelm version and updating again if needed. The last PR removes the covered TODO entry and extends the first
PR's fixed changelog fragment.

## Things you should know

- A real change to a host already running the helm's version can clear the warning on the next host-list read; it may
  barely appear. This is the clearing policy the maintainer chose. It does not establish how the detached update ended.
- If both ssh destination and identity change, the warning names the destination change. Messages stay lowercase with no
  final period, matching neighboring diagnostics. Values use the existing escaped display, and a missing identity reads
  "no identity reported". Losing an identity says the installation can no longer be identified, without claiming that a
  different installation was positively identified.
- An earlier unresolved update warning takes priority over the new version advice. A review found that replacing it with
  advice could otherwise erase the earlier uncertainty when versions matched. Both warning constructors now preserve it
  while still detaching the changed host's update. Other diagnostic text and clearing rules are retained.
- Version equality uses the UI's existing evidence about the helm: its reported differing build, or the UI build after a
  matching build header has been seen. Missing build evidence keeps the warning. The existing mismatch evidence
  remembers the first disagreement until reload and retains at most 64 characters. After another helm replacement or
  rollback without reloading, that evidence can be stale; an unusually long stamp cannot establish exact equality.
  Development builds that all report `0.0.0-unreleased` cannot be distinguished by version equality. This plan adds no
  separate version tracker.

## Open questions and possible follow-ups

None required for the agreed behavior. Tracking the latest helm build independently of the existing latched mismatch
would be separate work if the version-evidence limits above need to change.

## The PRs

- [#2501](https://github.com/scode/farhelm/pull/2501/changes): retain updates through the first learned identity and
  confirm the helm's identity-learning sequence.
- [#2504](https://github.com/scode/farhelm/pull/2504/changes): explain real changes, clear satisfied version advice, and
  preserve earlier update uncertainty. This draft builds on #2501; both remain open drafts.

## Checks run, reused and skipped

Runtime commands used `scripts/record-test-run.py`, pinned nextest 0.9.148 and the checksummed tmux 3.7c build. Rust
runs used four slots and zero retries. Browser selections ran both Chromium and WebKit, three configured workers (two
active projects), zero retries and one execution per selected case. All successful runs below have complete source,
runner reports and retained output, observed EOF, no runtime `SKIPPED` messages, and no cleanup limitations.
Selection-filtered Rust tests are not counted as execution.

- `cargo nextest run -p farhelm-ui -p farhelm-helm --lib` with the UI provisioning module and the single skewed-host
  update test selected: 25 passed in `5679689e-04dd-45ed-b2e3-5a3c17f68f9a`, on #2501's `50a0d766`. The joined helm test
  proves identity was absent before the update and recorded after its fresh reconnect. Its code and production
  dependencies remain unchanged in #2504, so that evidence is reused.
- `cargo nextest run -p farhelm-ui --lib`: all 593 then-existing tests passed in `b204db82-ac1f-461d-86c7-3c0893409701`,
  on the pre-review-correction revision `92508099`. The correction changed only warning construction and added a
  regression; the 566 tests outside the provisioning module remain covered by this run.
  `cargo nextest run -p farhelm-ui --lib -E 'test(provisioning::tests::)'` then passed all 28 affected tests on the
  final `33dfcddf`, in `75fe015c-7b42-4254-ae9c-6e052341ae4e`.
- `npx playwright test 'provisioning\.spec\.ts'` with explicit grep selections: 18 passed (nine per engine) for #2501 in
  `1ed905d2-7759-4d7e-b622-8b9474e3a6ae`; 30 passed (15 per engine) at `92508099` in
  `f74b975b-1770-473d-86ce-b2f1a0890c91`; and 24 passed (12 per engine) on final `33dfcddf` in
  `2218fbaa-4892-4c6e-b679-42c829011b7f`. The final selection covers both warning constructors, destination and identity
  values with invisible controls, disconnected/unequal/current versions, late acceptance after current facts, earlier
  uncertainty surviving a retarget, and existing acceptance/progress races. The first-identity scenarios and the
  separate progress/submission race remain covered by the earlier 30-case run: the correction's priority check leaves
  their empty-warning case unchanged.
- `cargo fmt --all -- --check`, `cargo clippy --all-targets -- -D warnings`, `git diff --check`,
  `python3 releasing/check-changelog.py format`, and `dprint check TODO.md` passed. The isolated
  `python -B scripts/check-test-sleeps.py` passed with 307 delays inspected and zero unexplained. The final correction
  reran formatting, Clippy, diff and delay checks; the unchanged fragment and TODO formatting reuse their earlier
  passes. The Dioxus 0.7.10 release web build passed on the final source. The unchanged server build from base
  `04d52765` is reused because neither PR changes server production code.

The stack remains based on `04d52765`. The complete upstream diff through `efb32081` was inspected: it changes only
queue state and other plans' reports, with no product, specification or dependency interaction. No source rebase or
additional runtime check was needed for that movement.

An initial browser recorder invocation from the wrong directory refused before starting a child, retained as
`d1fac012-9a2c-44a4-8e2a-7c085ac5766a`; correcting the working directory produced the successful run above. An initial
web-build invocation selected an unrelated tool named `dx` and failed before compiling; explicitly selecting Dioxus
corrected it. Neither is runtime or latent-flake evidence. TypeScript and fragment dprint checks were inapplicable to
this repository's configuration, and were not counted as passes.

The full browser spec and workspace runtime battery, desktop runtime, installer, provisioning-container and website
checks were skipped: the change is confined to host-update continuity and warning presentation, covered by the selected
UI contracts, two browser engines and the joined helm test. No instruction, heading or file-location changes required
the agent-instructions checker. CI is manual-only and was not dispatched.

## Review gate outcome

Each PR received its single fresh gpt-6.1-sol/high correctness, design and idiom review with the complete goal and test
authoring checklist. #2501 had no findings. #2504's earlier-warning preservation finding was accepted, corrected in the
same PR, and verified by the same reviewer with no new findings; the final runtime checks passed afterwards. There were
no rejected findings or material scope departures. Commit and PR wording passed their separate fresh cold reads.

### Landing

Landed as #2501 and #2504, merged bottom-up, with the same content the report describes, rebased onto this round's main.
It was the last of five plans in this landing round, after ui-session-actions, connection-stall-phase,
never-started-launch-cause and yolo-confirm-launch-guarded.

#### What landed since the stack was based, and whether it interacted

Since the stack's base, main gained helm-ingress-types (typed request parsing in the helm, and tighter rules on how the
helm reports errors that came from a supervisor) and create-reservation-retention (a create's retry key is freed when
its session is deleted). A fresh-context reviewer and I read them, and the other four plans of this round, against this
plan.

- connection-stall-phase, earlier in this round, changed what a helm connection attempt reports and added a live
  connection stage to the host list. This plan's helm test, which checks that updating a protocol-skewed host records
  its identity, works with the extended connection result; the UI's update-following ignores the new stage field, so a
  stage change neither detaches an update nor raises the warning.
- helm-ingress-types does not change how the helm records a host's identity or reports host facts, which is what this
  plan's warning reads.
- ui-session-actions, never-started-launch-cause and yolo-confirm-launch-guarded do not touch hosts or updates.

#### Fixes made during the landing

None to the plan's code. Its TODO.md removal conflicted with neighboring changes on main; the landing kept both sides.

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
the full Rust runtime suite and the full browser suite; both ran this round, above. Nothing in the executor's report
became untrue at landing.

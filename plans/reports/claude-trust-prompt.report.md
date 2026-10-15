## What this was about

Claude asks whether to trust each fresh managed checkout. The maintainer chose an off-by-default setting that answers
this by editing Claude's own settings, with clear warnings about that unsupported mechanism, and a discoverable hint
above the unanswered question.

The setting covers every new managed checkout, including agent-requested ones and checkouts initially launched with
another agent. It never marks a user-selected folder trusted. The hint appears only for a current Claude trust question
in a managed checkout while the setting is off; it leaves terminal input and keyboard focus alone. Settings restores the
shared hint preference; × hides only that session's current occurrence, including across navigation.

## Things you should know

This is best effort. Farhelm changes only the checkout's `hasTrustDialogAccepted` key, shares Claude's directory lock,
and atomically replaces the vendor file while preserving its mode and symlink. A simultaneous unlocked Claude exit save
can lose an update, and a future Claude version can change the format or locking without notice. The Settings caution
explains these risks next to the switch. A failed edit does not prevent agent launch.

Installed Claude 2.1.300 confirmed the configuration format and trust dialog in isolated throwaway state. Execution
found that Claude explicitly locks the selected configuration filename plus `.lock` even when that filename is a
symlink; the writer follows that observed alias while replacing the resolved target. Configuration resolution also
accounts for legacy and custom-OAuth filenames. All Farhelm processes and Claude files used for validation were
disposable fixtures.

Archive or confirmed missing-folder retirement schedules removal of that one trust key outside Delete's reply path. A
refused archive retains it. Cleanup holds path admission until the filesystem worker finishes, so a later checkout
cannot reuse the path while an old remover is still editing its key. Failed cleanup retains the removal record for
startup recovery; cleanup is also best effort. A failed removal can leave existing vendor trust behind, and turning the
setting off does not revoke it.

The helm database advances to schema 53. Older releases refuse this database rather than downgrade it; no data is
discarded by that refusal. Protocol 51 requires supervisors to be upgraded together with the helm before they can
reconnect. Changing the trust setting leaves open checkout path previews valid because it changes no destination; create
and retry freeze the selected value.

## Open questions and possible follow-ups

There is no maintainer decision blocking this work. Compatibility with future Claude versions remains a maintenance
risk, explained in the product. The optional full manual real-Claude create/archive walkthrough was not run; isolated
vendor research, actual launch-child tests, cleanup lifecycle tests, and browser tests cover the corresponding
boundaries without authenticating an agent or submitting a model request.

## The PRs

- [#2070](https://github.com/scode/farhelm/pull/2070/changes) — opt-in managed-checkout Claude trust and its discovery
  hint; one coherent draft PR.

## Checks run, reused and skipped

- Passed: `cargo fmt --all -- --check`, `dprint check`, whitespace check, changelog format, and the isolated test-sleep
  checker (301 delays, zero unexplained). These inspect the changed source, documentation and fragment.
- Passed: the affected supervisor/protocol/helm/UI selection through recorded nextest, four slots and zero retries, with
  exact pinned tmux: 248 tests, recorder `e633c544-e3a0-4b3d-866c-b335a1c3dfbc`. This covers trust preservation,
  symlink/mode, lock retry/refusal, launch opt-in/default/best-effort behavior, archive/refusal/missing/startup cleanup,
  detection, schema, preferences and HTTP contracts.
- Passed: CLI/fixture debug build and release web build. Final hint screenshots were inspected for placement, readable
  caveats, focus and terminal space.
- Passed: six targeted browser cases on Chromium/WebKit, recorder `8152b23b-ac9f-4094-97ea-5ee40bfab7d4`, covering the
  hint and Settings layout/focus isolation; after applying existing button styles, both feature cases passed again
  against the rebuilt web UI, recorder `3e52c828-7a8f-4b46-980c-d9985e12095c`.
- Passed: `cargo clippy --all-targets -- -D warnings` and `cargo clippy -p farhelm --bins -- -D warnings`, covering both
  the test-enabled supervisor and shipped binary configuration.
- Passed: the final cancellation test after correcting its OS-lock readiness oracle, one test, recorder
  `d790674b-61aa-4c84-8b12-b75f0343a16f`.
- Passed: four managed-checkout process scenarios covering real clone/hook/agent execution, creation, retry replay and
  archive, recorder `8f873778-3b56-48fc-8ac5-31c6173739d1`. Its terminal client exited unexpectedly, but the owned
  recorder continued: its completed manifest reports exit zero, complete retained output/JUnit and complete pinned-tmux
  lifecycle. The JUnit count includes the successful fixture-build setup in addition to the four tests.

The initial browser run (`7640ce05-982b-453b-89a3-e548ee3a7060`) retained five fixture failures: the Settings row-count
assertion still expected eight switches, its old tall viewport no longer fit the added caution, and WebKit's `check()`
action required a checked result after an intentionally refused save had correctly rolled it back. The fixtures now
expect ten switches, establish a sufficiently tall viewport while preserving the fit assertion, and click for the
rollback test. The later six-case run passed. No selected browser case was skipped; the absent optional remote SSH
fixture is outside these local-session cases.

The first targeted run retained two failures in historical migration fixtures: the artificial rewinds had left the new
preference column in place before replaying older migrations. The fixtures now remove that later column, and fresh
schema column order matches the upgrade path. Assertions were preserved. The focused two-test reproduction passed
(recorder `348f7364-3903-43b3-b530-6f412a4fca64`); the original failed run remains retained as
`a26fc0fa-b74a-4410-97af-62b800e3e11d`. One process-test build started during a helper move and failed compilation
before tests ran (`2ec475e5-ace0-4616-82c8-3eb9e4ef3c87`); its later complete-source run is listed above.

Reused: the successful 32-page website build after the documentation changes; subsequent edits affected Rust, browser
proof, and code comments only. Browser runs reused the first successful CLI/fixture build: the intervening binary change
only retains startup cleanup ownership, while these fixtures have no retained trust-removal records and do not cancel
construction. The final CLI build and cancellation/process checks separately cover that change. Earlier compiler checks
supplied development feedback; final targeted runs supply the runtime verdict.

The final commit is `69bff369611a653c0cf9beb81e42606e79fa093c`. Recorded tests used the feature working tree over
claimed main `7fcee562`; the careful rebase onto `bb22a7a7` read the entire intervening diff, which only added a
quick-launch-template TODO. The adjacent TODO conflict kept that addition and removed only the completed Claude-trust
entry. Executable behavior and test coverage are unchanged, so no runtime repetition was needed for the rebase.

Skipped: the full workspace/browser batteries, desktop native runtime, installer, release and provisioning gates. This
change has concrete launch, archive, protocol, migration, shared-setting and browser risks, covered by the targeted
selections above; it changes no installer, release, provisioning or desktop-shell behavior. No website deployment was
requested or performed.

## Review gate's outcome

The required fresh source review found wrapped-screen detection, occurrence dismissal lost on navigation, and a
contention test without an actual retry witness. All were corrected. Corrective reviews also strengthened accepted-state
browser witnesses and the startup cancellation test's OS-lock readiness oracle. The reviewer found no remaining product
correctness issue in the final lifecycle correction. A fresh scope reassessment found no unnecessary product complexity;
its single shared-dismissal proof gap was corrected with the existing witness. Final fixture repairs preserve the
original assertions and test the intended boundary. The wording cold read passed.

Implementation stayed local under no-workhorse mode. The prescribed source review used gpt-6.1-sol at high effort
through an independent Codex process. Commit/PR wording received a fresh gpt-6.1-sol medium cold read. The scope
reassessment used a fresh native reviewer on the executing session's model. A fresh native cold read checks this report
before delivery. Private galaxy session `d389963b-3fd4-4459-94ac-1ab17a206b5d` retains routing and available usage
evidence under the standard galaxy session-record location. Source-review counters are available; native and
orchestrator usage are unavailable, and some launch/decision records were written after the launches. No complete cost
total is claimed.

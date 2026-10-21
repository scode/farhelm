## What this was about

Create retry keys kept accumulating after their sessions were deleted. The maintainer chose session lifetime for every
create, including app and agent requests, and a 24-hour replay window for creates refused without a surviving session.
Delete now frees the key transactionally, including an unfinished create's key. A later retry creates a new session;
retries while the original session exists keep their existing recovery behavior.

## Things you should know

An agent's create or clone after Delete still needs the user's approval. Retained error sessions keep their keys until
Delete. Refused creates keep their original request fingerprint and error for 24 hours, then retry as fresh requests.
Expiry runs before a keyed request is resolved and when a writable supervisor opens the store; an idle database can
physically retain expired refusal records until then.

Opening an existing store removes old records for deleted sessions, converts live permanent keys, and expires old
refusals. This cleanup is irreversible on shared devbuild state: returning to an older build cannot restore the removed
deleted-session answers. The database schema version is unchanged, so older supervisors can still open it. Unresolved
legacy pending records retain their recovery evidence.

The implementation notes record the reversal of the July keep-forever decision and its rationale; historical records
were left intact. The covered TODO entry is removed.

## Open questions and possible follow-ups

None required for this plan.

## PRs

[#2497](https://github.com/scode/farhelm/pull/2497/changes): create keys follow session lifetime, refusals expire after
24 hours, and existing records adopt that policy.

## Checks run, reused and skipped

Recorded Rust checks used nextest 0.9.148, tmux 3.7c, four global slots and zero retries. Run records have complete
source identity, JUnit reports and retained console output. Commands below are the child commands passed through
`python3 scripts/record-test-run.py --runner nextest --kind development --tmux required` with explicit selection and
concurrency labels and a timeout.

- `cargo nextest run -p farhelm-supervisor --lib -E 'test(store::tests::) | test(service::handlers::tests::) | test(service::core::tests::)'`
  passed 425 tests in run `3017b9f3-542a-493b-93ad-be7983afeab6`. This covers row conversion, exact refusal expiry,
  retained error sessions, Delete races, refusal proof and managed-checkout recovery. Its source was base `c594ff95`
  plus working diff fingerprint `05221bd20750c93e87ad79a385de2601314abf14c59270a00554cd73aacc2e66`. Reused after the
  final delta because production behavior was unchanged; the strengthened legacy test was run again below.
- `cargo nextest run -p farhelm -p farhelm-supervisor --test e2e --lib -E 'test(=create_idempotency::deleting_a_pending_create_frees_its_key_without_relaunching_its_identity) | test(=service::core::tests::a_legacy_pending_orphan_refusal_uses_the_current_lifetime)'`
  passed both corrected or strengthened cases in run `c59d1665-b2f9-422f-935b-17bed476e58c`.
- `cargo nextest run -p farhelm --test e2e -E 'test(create_idempotency::) | test(agent_relay::) | test(=agent_listing_real_stack::keyed_agent_create_and_clone_need_fresh_approval_after_delete) | test(=agent_listing_real_stack::browser_link_cli_waits_without_fencing_and_reports_each_answer)'`
  passed all 24 cases in final run `3a5076d0-41c3-496a-bc10-1356c3727280`, including fresh create and clone approval
  cards after Delete, crash recovery and pending-key deletion. Both final runs used base `c594ff95` plus working diff
  fingerprint `11d32f54ca2611a82448835c74ebd4e0e1f87184fc7ba4f9a3fb101b67ab3fce`; no source edits followed. Retained
  output has no runtime `SKIPPED` markers; selected-out tests are separate from passes.
- The first 24-case end-to-end selection, run `6104878b-647c-4d75-ad42-11f0480bffa8`, had 23 passes and one failure.
  Exact reproduction `cc07955d-4fcb-49c2-a42e-3b9112d2a348` failed the same new test. Its crash seam precedes
  publication in the supervisor's session list, so the fixture attempted Delete too early. It now uses the existing
  restart handoff and verifies the pending session is listed before Delete. Both failed records remain retained; no
  latent flake is claimed. JUnit also counts the successful fixture-build setup case separately from these test totals.
- `cargo clippy --all-targets -- -D warnings` and `cargo clippy -p farhelm --bins -- -D warnings` passed, covering both
  test-enabled and shipped configurations. `cargo fmt --all -- --check`, scoped `dprint check`, and `git diff --check`
  passed. The test-delay checker passed with 307 delays inspected and none lacking a rationale.
  `python3 scripts/check-agents-md.py` passed with no errors or warnings, and
  `python3 releasing/check-changelog.py format` validated 66 fragments. The last two checks were reused after the final
  comment/test delta because it changed no headings, instruction links or fragment content; the other checks ran on the
  final source.

No previous plan's checks were reused. Browser, desktop, installer, release and full-workspace runtime suites were
skipped because this change affects supervisor storage and create recovery; the selected service and real CLI/helm tests
exercise those contracts without changing browser or desktop code. No hosted CI run was requested.

## Review gate outcome

The required single fresh gpt-6.1-sol reviewer at high effort reviewed correctness, design, language idioms and the full
test-authoring checklist. It found no functional or design issue and one stale-comment issue. That was corrected; the
same reviewer checked the final documentation and test delta and reported no findings. Commit and PR wording passed a
fresh cold read.

### Landing

Landed as #2497, with the same content the report describes, rebased onto this round's main, with one test conflict
resolution described below. It was the second of two plans in this landing round, after helm-ingress-types.

#### What landed since the stack was based, and whether it interacted

Since the stack's base, main gained ui-peer-text-type (UI only), uninstall-refusal-wording (helm uninstall refusals) and
plan bookkeeping, and helm-ingress-types landed just before it in this round. A fresh-context reviewer and I read them
against this plan. This plan changes no helm code, but its replies still pass through the helm's reply decoding that
helm-ingress-types restricted, and the helm classifies them as before: a retry after a delete becomes an ordinary new
session, and a create refused within its 24-hour window returns the same error the user first saw. The helm never
repeats a keyed create on its own after a refusal, so refusals now lasting 24 hours change nothing in the helm.

#### Fixes made during the landing

- The end-to-end create retry test file conflicted with helm-ingress-types, which had switched an error check in the old
  "a retry after delete is refused" block to the error type's new accessors. This plan replaces that block entirely,
  because a delete now frees the key, so the landing took this plan's version. Its new checks do not read the error
  type, and the rest of the file already uses the accessors.

#### Possible follow-ups

- Found at landing; neither the report nor the executor's review flagged these: five code comments outside the
  supervisor still describe the old rule that a create's key lives forever unless it is marked to live with its session:
  the protocol's documentation of that marker, the helm's notes on spawned sessions and on agent spawns, the UI's note
  that the marker proves a permanent refusal, and a protocol note about permanent checkout reservations. The supervisor
  now ignores the marker, as SPEC_impl.md says. They are comment-only and change no behavior; possible follow-up: update
  them.

#### Checks

Together the two plans change how the helm parses every request and attributes errors, and how the supervisor keeps and
frees create retry keys, about 3,000 added and 1,700 removed lines; that reach is why this round ran the whole Rust
workspace and the full browser suite. All of the following ran on both plans stacked together on this round's main,
including the test conflict resolution:

- `cargo fmt --all -- --check`, `cargo clippy --all-targets -- -D warnings`,
  `cargo clippy -p farhelm --bins -- -D warnings`, `cargo check -p farhelm-ui --features desktop`, the isolated
  test-sleep check, `dprint check` on every changed Markdown, JSON and TOML file,
  `python3 releasing/check-changelog.py format`, `python3 scripts/check-agents-md.py` and
  `scripts/check-desktop-assets.sh`: all passed.
- `cargo nextest run --workspace --exclude farhelm-desktop` through the test recorder with pinned nextest and tmux, four
  parallel test slots and no retries, run `1719f6e8-158f-4c4f-aeb5-1beb7448cc68` (kept): 4014 passed, 1 failed:
  `terminal_backpressure::shallow_pause_resumes_without_reset_or_replay`, which has an open TODO deflake entry ("Shallow
  pause resets under load") and fails nine runs in ten on main on this machine without these plans (see
  supervisor-request-modes' landing notes). create-reservation-retention changes the supervisor, so this result cannot
  show whether it affects that test; it changes how the supervisor stores and frees create retry keys, not terminal
  output flow, so I attribute the failure to the known flake.
- `cargo nextest run -p farhelm-ui --features desktop`, run `399561a0-e400-41bc-baf4-e3851c200847`: 750 passed.
  `node --test` in `crates/farhelm-ui/js-tests`, run `9223f292-2f47-4978-835d-da43554bc77b`: 328 passed.
- The full browser suite (`npx playwright test` in `e2e/`) on Chromium and WebKit, three workers each with its own helm
  and supervisors, no retries, run `cd9c8375-7d5e-463e-ac6b-fb2d96014aa3` (kept): 2230 passed, 14 skipped by design
  (WebKit clipboard cases and tests that need a real signed-in agent or run on one engine only), 2 failed:
  - on Chromium, "terminal link ownership opens once and preserves program clicks in all mounted terminals" timed out
    waiting for its test agent to start;
  - on WebKit, "don't ask again on a refused YOLO replace marks the host, then replaces with the override", the case
    FLAKES.md recorded as a flake on 2026-10-20.

  Three repeats of each on both engines then passed (run `775ade88-df3c-4bc8-81ed-c744911ccf8b`). I attribute the
  Chromium timeout to machine load from three parallel workers: it passed three repeats on both engines, and the rest of
  the suite starts test agents the same way. I did not add it to FLAKES.md on a single failure. The WebKit failure is
  the flake FLAKES.md already records.
- Skipped: the workspace and desktop doctests (no documentation example changed), `cargo check -p farhelm-desktop` and
  the desktop smoke test (no desktop code changed), the native Mac app (not runnable here) and the installer
  (unchanged).

The report's checks ran on a main from before ui-peer-text-type and uninstall-refusal-wording landed, and it skipped the
full Rust runtime suite and the browser suite; both ran this round, above. Its note that no hosted CI run was requested
still holds. Nothing else in the executor's report became untrue at landing.

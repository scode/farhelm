# A rail bell makes failed Trash cleanup visible

## What this was about

A deleted checkout could leave Trash immediately even when its files later failed to be removed, so disk space stayed in
use with only a logged warning. The maintainer chose a general bell for notices about hosts and Farhelm, starting with
these cleanup failures, plus Retry in both the bell and Trash.

The bell sits above Hosts in the navigation rail. It turns red with an unread count, lists the affected host and
checkout, and offers Retry now and Open Trash. Closing marks only the items shown as read. A repeated failure updates
the same notice quietly; confirmed removal resolves it, and Clear resolved removes its history. Windows sharing a helm
share those marks and history. Trash retains removing and failed checkouts in its count and known size totals.

## Things you should know

Cleanup can already have removed some files before it fails. Retry repeats the ordinary identity, session and mount
checks; Farhelm does not repair permissions or use elevated privileges. Each supervisor start also retries retained
cleanup. An older host keeps its prior behavior until updated and never supplies a false resolution.

A connected client observes failures through the existing Trash reads and fleet updates. A client that did not start
cleanup still discovers its progress, even if the initiating window closes. Slow hosts have independent bounded readers
so a burst of updates cannot queue unbounded requests or hold another host back. No new background observer, channel or
idle polling loop was added. Unreachable hosts retain their last known notices; removing a host removes its notices.

Read marks follow the existing session bells' best-effort behavior: a failed mark can leave the bell loud until the
panel is closed again. A notice-storage failure does not hide a successfully read Trash listing. The user docs updated
are Read the session list and Manage hosts; no preview server was started during this unattended run.

## Open questions and possible follow-ups

None required for this plan. Only the stuck-Trash notice kind ships. Host setup attention, announcements, session bells,
sounds and ntfy remain separate.

## The PRs

- [#3243](https://github.com/scode/farhelm/pull/3243/changes) — retained cleanup facts, guarded Retry and the Trash
  rows.
- [#3244](https://github.com/scode/farhelm/pull/3244/changes) — shared notice history and read/resolution endpoints.
- [#3251](https://github.com/scode/farhelm/pull/3251/changes) — the rail bell, shared actions, bounded refresh,
  specifications, user docs and source TODO removal.

All remain drafts for the landing monitor. The first two outline steps were combined so the host's new facts and their
Trash presentation form one working review unit.

## Checks run, reused and skipped

- `cargo nextest run -p farhelm-ui --lib -E <notice and Trash selection>` passed 14 cases, four nextest slots, zero
  retries, no tmux needed; recorder run `8ddc55ed-5cc4-487d-a9f9-6a18174e06a5`. This includes exact shown-ID marks,
  quiet resolution, current host/entry Retry eligibility, accepted cleanup retention and batched trigger authority. The
  later panel input-style correction does not change those pure helpers; the browser gate covers that correction.
- `npx playwright test <rail, Trash, real archive and sidebar selection>` passed all 58 cases: 29 Chromium and 29
  WebKit, three workers with separate stacks, zero retries and no skips, flaky outcomes or global errors; recorder run
  `246332f0-494a-4939-ba71-c2c7cee8f4de`. Both report and retained output are complete. This covers palette fills, exact
  close marks, focus and outside dismissal, Retry/Open Trash while marking or managing hosts, bounded independent host
  reads, real archive deletion, removal polling and sidebar geometry. Twelve quiet, alerting and resolved images across
  both palettes and engines were inspected and retained with the private visual companion. Its images come from the
  earlier run's successful bell cases; the bell source and fixture are unchanged in this final pass.
- Native UI all-target compilation, matching CLI and fixture builds, pinned dx web build, changed-file formatting,
  changelog format, whitespace and the isolated test-delay check passed. The delay checker inspected 430 delays with no
  unexplained one. `cargo clippy -p farhelm-ui --all-targets -- -D warnings` passed after correcting two style lints.
  The docs build passed and validated all internal links.
- Reused supervisor/protocol evidence: parent isolated run `b11a2226-8bb9-450f-a444-2abdbe0cc8a9` passed 42 cases;
  integrated run `bec335fe-8303-4b8b-ae4e-54f9d210c5bb` passed 54 selected protocol, supervisor, helm and Trash cases.
  These overlap. Tests include actual non-root permission refusal, Retry, startup recovery and retained accepted
  journals. The read-only-directory permission premise does not work as root; this run was non-root. No runtime
  `SKIPPED` occurred. The first draft's reviewed source is unchanged through restacking; final browser checks cover the
  later UI changes.
- Reused shared-history evidence: parent isolated run `9929db7e-13c2-4585-9306-7767c7ba8e66` and integrated run
  `4c660a8c-ec40-4097-b72d-4ea3a55ef193` each passed 12 selected helm tests with temporary SQLite and controlled
  supervisor connections, four slots and zero retries, no runtime `SKIPPED`. These cover persistence, repeated failure
  without re-alerting, resolution evidence, exact marks, host removal and ordering with reconnect/retarget/Delete. The
  second draft's source is unchanged through restacking. Earlier per-unit Clippy and shipped-binary checks remain
  recorded; the final unit adds no shipped-binary feature branch.
- Earlier browser run `8c38c028-9535-4c4e-8f93-113b3cb0017d` passed 42 cases across Chromium and WebKit on the first
  unit, including real archive cleanup. The final selected gate repeats the affected Trash and real-archive coverage;
  this earlier count is a separate observation, not added to it.
- Same-session failures were retained: the initial supervisor selection found an errno-classification defect, corrected
  before its successful narrow and full runs; two helm fixtures needed their actual connection-event and lazy-row
  premises corrected; an initial browser startup had no fixture binary and ran no tests. These are development
  observations, not latent-flake claims. The first final-browser command, run `068460a0-6f32-472e-8845-a84e50c98a5b`,
  had 53 passes and five failures in three existing Trash tests. Its fixtures assumed host news was already applied,
  allowed an unintended fallback during guarded clicks, and waited for a new read while holding its single-flight
  predecessor. The corrected tests isolate those boundaries without dropping preservation or mutation-fencing
  assertions. Exact WebKit and Chromium reproductions passed before the final gate; the same reviewer also found and
  verified correction of a clock/network timing edge in the polling test. All failed evidence remains retained; the
  failed command is not used as a pass.
- Skipped the full workspace, desktop runtime, installer, provisioning and old-terminal suites: the changes concern
  archive observations, shared history and UI behavior, covered by the targeted checks. WebKit supplies browser-engine
  coverage, not a native macOS desktop run. Compatibility is evidenced by additive-field decoding, not an older
  supervisor binary. Hosted CI was not dispatched.

The final browser run captured `83a9761d` plus source diff `4006161b6ed6`; the 14-case UI run captured the same base
plus `74c59d1afad0`. Backend integration captured `68c83ef1` plus `eba5b217026e` (54 cases), and `da80dc1a` plus
`1dbab6cab80b` (12 helm cases). The recorder retains the full source identities. Subsequent changes were the panel
input-style correction covered by the final browser run, equivalent lint rewrites, test-fixture corrections covered by
that run, and restacking.

The stack was reconciled onto main at `b0893258`. Source review covered the newly landed Stop-during-start behavior,
upgrade floor and deterministic terminal sizing, including their shared host identity, protocol, storage and layout
boundaries. They leave the cleanup facts, notice authority and bell readers unchanged. The only textual conflict was
adjacent completed TODO entries: both removals were kept. All other feature hunks are unchanged. No extra runtime run
was needed for that rebase; this does not claim the earlier runs executed the newly landed features. Instruction links,
changelog format and changed Markdown formatting passed after reconciliation; the instruction checker reports one
existing missing-heading warning in the triage history, with zero errors.

## Review gate's outcome

Each of the three review units received the required single fresh-context gpt-6.1-sol/high review. The final review
found that the panel would retain pointer-events:none after becoming visible. Both visible placement branches now reset
it explicitly; the same reviewer verified the correction and found no remaining issue. Computed pointer-events and
ordinary pointer clicks on its actions passed in both engines. Fresh scope assessments approved the minimal
existing-reader reuse and the earlier cleanup ordering correction; no required behavior was dropped. Separate
documentation passes and commit/PR wording cold reads are complete.

### Landing

Landed together with mux-chunk-size (smaller chunks for bulk transfers through the helm, with a protocol version bump),
stacked below it. Main had just gained cheaper cleanup for sessions in systemd scopes, and checkout folders set per host
(the per-host-checkout-folder plan). A fresh-context review read them against each other:

- This plan's new notice fields are optional, so a new helm talking to an older supervisor never resolves a notice or
  offers Retry wrongly.
- Its helm database upgrade and fresh-database paths match.
- Trash cleanup removes folders and the scoped cleanup ends processes, so they do not overlap.
- The per-host checkout folder change touches the same files in different functions.
- The rail's footer grows from 168px to 208px. Nothing else depends on the old height, and neither Settings nor host
  settings is affected.

#### Fixes made at landing

- per-host-checkout-folder had edited a doc comment in the UI's API code right where this plan adds its notice code. The
  textual conflict was resolved by keeping both (#3251).
- A test already on main turns a current helm database back into an old release's layout and compares the result with
  that release's recorded layout. Its list of what to remove did not include this plan's new notices table, so two
  checkout-configuration tests failed once both changes were combined; the table is now on the list (#3244).
- The Session list docs page and the changelog entry said "Older hosts keep their previous behavior until updated".
  mux-chunk-size raises the protocol version in the same round, so an older host no longer connects to this helm at all:
  it shows as needing an update. The sentence is gone from both (#3251). The executor's report says the same thing ("An
  older host keeps its prior behavior until updated") and is now wrong on that point.

#### Things you should know

- One judgment call, mine as the landing monitor, landed as is and is recorded in TODO.md's "Plan decisions to revisit".
  Whenever any session's status changes, and on the client's periodic catch-up refresh, each open window now re-reads
  every connected host's Trash. Each read uses one of the host's two slots for folder browsing, which the launcher's
  folder picker also uses, so folder browsing can feel slower while the fleet is busy. Each read also takes a per-host
  lock on the helm and a database transaction, even when nothing is stuck. The executor's report says no polling loop
  was added, which is true of timers but not of this traffic. It is bounded, about one read in flight per host per
  window, and does not feed on itself. A cheap mitigation is to skip the lock and the transaction when nothing is stuck
  and the host has no unresolved notice.
- If mux-chunk-size were ever reverted before a release, a helm on the previous protocol version could show a folder
  whose deletion is already in progress as an ordinary archive you can restore, and restoring it would hand back a
  partly deleted checkout. Re-check this plan in that case.

#### Checks

The two plans were checked together, stacked in queue order on main (rail-notification-bell's top commit, then
mux-chunk-size's), on this development machine and, at the maintainer's request, on Tensorlake sandboxes. The landing
resolved the UI API conflict before any check ran, so every run below includes it. The merged tree is the one checked
last: rail-notification-bell's top at `94a5b4f7` and mux-chunk-size at `1ffbd128`. Main after the merges differs from it
only by two changes that landed meanwhile and touch neither plan: a queue-picking rule for plan executors, and
review-queue bookkeeping.

On the stack before the fixture and docs fixes (rail-notification-bell's top at `68da3700`, mux-chunk-size at
`e5fd6349`):

- Here: `cargo fmt --all -- --check`, `dprint check`, `python3 releasing/check-changelog.py format`, the test-sleep
  checker, `cargo clippy --all-targets -- -D warnings`, `cargo clippy -p farhelm --bins -- -D warnings`,
  `cargo check -p farhelm-ui --features desktop`, the docs website build and the UI's JavaScript unit tests passed.
- Tensorlake, browser: `rail-notices`, `trash-dialog`, `header`, `sidebar`, `tensorlake-tunnels`, `terminal-files`,
  `terminal-links` and `agent-relay` on Chromium and WebKit over eight sandboxes: 372 passed, none failed, skipped or
  retried. Runs `dee09cd5`, `6806c67b`, `0a0124fb`, `986657df`, `04198f20`, `8ef40cc8`, `b20d7f3e` and `d6fba799`, in
  the recorder's plain command mode (its Playwright mode cannot pick one engine per file) under the sandbox's slightly
  older Node 24. Every sandbox reported its per-user systemd manager and its SSH to itself working.
- Tensorlake, Rust: every Rust test in the workspace except the desktop app's (the sandboxes cannot build or run the
  desktop app, and neither plan changes desktop-only code beyond what the desktop compile check above covers), from one
  archive over nine sandboxes. Runs `3dd29160`, `eaaa34eb`, `b32f6ffc`, `48183d95`, `4f412bce`, `96e65397`, `614967f5`,
  `3e110a64` and `3ca3aec9`. There were five failures:
  - two in the helm's checkout-configuration tests, fixed by rail-notification-bell's schema-fixture landing fix;
  - `attribute_compile_contract_matrix`, which compiles small crates with a nested `cargo` that the sandboxes do not
    have. Neither plan touches its crate; it passed here in an earlier round on a tree that differs from this one only
    outside that crate (run `05991d87`);
  - `create_idempotency::a_crash_before_the_outcome_commit_still_replays_the_same_session` (run `614967f5`, retained).
    Delete refused because the sandbox's per-user systemd manager stopped answering its probes. This is the same failure
    as in the previous round, in a test neither plan touches, and it is being recorded as a latent flake on its own;
  - `service::core::tests::a_failed_create_replays_its_error_and_a_changed_override_conflicts` (run `eaaa34eb`,
    retained), whose create reply did not arrive within 10 seconds on a loaded sandbox: one observation, in code neither
    plan touches.

After the fixture and docs fixes: formatting, `dprint check`, the changelog format check,
`cargo clippy -p farhelm-helm --all-targets -- -D warnings`, the website build, and recorded
`cargo nextest run -p farhelm-helm` here. 1659 passed; the two skipped are the real-Mac tests that only run by hand (run
`ad5b842d`). That covers the fixed checkout-configuration tests and every helm test of the new notices. Nothing else was
rerun: those fixes changed one helm test fixture and two sentences of documentation.

Reused: nothing from the executors; the round's own runs above cover both plans on the combined tree.

Skipped: the rest of the browser suite (the specs above cover the bell, Trash, the rail and the relay paths), the
desktop smoke (no desktop-only code changed beyond what the desktop compile check covers) and hosted CI (manual-only,
not dispatched).

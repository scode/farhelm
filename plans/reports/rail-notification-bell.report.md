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

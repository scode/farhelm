### What this was about

Two review findings described possible file loss after a power cut: a downloaded copy whose save had reported success,
and a new managed checkout whose enclosing folders had just been created. Neither loss was reproduced. The maintainer
decided that downloaded copies and managed checkouts need not survive a power loss or system crash. This plan records
that decision and closes each finding in its own draft PR.

### Things you should know

The exemption explicitly covers downloads and managed checkouts, even immediately after success. Critical state such as
the installer and the helm's and supervisors' own SQLite databases stays subject to its existing contracts; no new
durability guarantee is added. The existing prohibitions on database corruption, bricking an installation and similar
lasting damage remain intact. The attachment upload's separate best-effort power-loss rule is unchanged.

No product code changes. The download and checkout operations still behave as before; this resolves the specification
and review tracking, without adding filesystem flushes or proving crash persistence. Each retained outcome records its
change, bookmark and draft PR, and both feedback files and index entries are removed.

### Open questions and possible follow-ups

None within this plan. A future request for stronger crash persistence would change the accepted policy; these PRs do
not make or verify that improvement.

### The PRs

- [#2039: accept download and checkout loss after a power cut](https://github.com/scode/farhelm/pull/2039/changes): the
  dated specification decision and download finding closure, draft against main.
- [#2043: record accepted checkout loss after a power cut](https://github.com/scode/farhelm/pull/2043/changes): checkout
  finding closure covered by #2039, draft against its bookmark.

### Checks run, reused and skipped

- `dprint check SPEC.md TRIAGE_OUTCOMES.md review_feedback_queue/INDEX.md` passed, including after both same-change
  PR-link updates. A separate documentation pass covered all touched files. Each PR was checked to contain only its own
  outcome's intended files and no `plans/` changes; both draft states, bases and exact pushed heads were verified.
- No earlier validation results were reused. The complete incoming diff since base `225dc101` concerned other queue
  claims and the plans watcher's idle-wait policy. It changed no specification, outcome, feedback or index contract this
  stack relies on, so no rebase was needed.
- Rust, JavaScript, browser, installer, desktop, release and power-cut runtime tests were skipped. These PRs change only
  ordinary Markdown, with no generated artifact or executable example, and the plan requires changed-file formatting
  checks only. No product build or live application was run. No validation failed.

### Review gate outcome

Each PR received its required fresh-context gpt-6.1-sol high review through codex exec. Both reviews reported no
actionable findings. They checked the complete patch, the full agreed outcome, the exemption's limits, the
critical-state and attachment boundaries, and the queue/ledger bookkeeping. They were static reviews and ran no tests.

The commit and PR wording passed separate native gpt-6.1-sol medium cold reads after clarification. Implementation and
investigation stayed in the main session under the no-workhorse requirement. External per-turn counters and review
artifacts are retained in the private galaxy-brain session record; exact reported model attribution, native/main usage
and some route-at-launch records are unavailable. The report also receives its required native cold read before
delivery. Neither PR was marked ready or merged by the executor.

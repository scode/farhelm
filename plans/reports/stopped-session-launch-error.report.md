### What this was about

Stopping a session while its checkout was being cloned or its post-clone command was running could leave the session
showing Error indefinitely. The supervisor had successfully recorded it as stopped, but a late preparation failure kept
overriding that result whenever the session list refreshed.

The approved fix keeps a recorded stop authoritative. Session replies now ignore a cached preparation failure once that
run's recorded outcome no longer permits it. Recorded errors keep their own explanation, and preparation failures remain
visible for runs whose outcome can still accept that evidence.

### Things you should know

The change is confined to status replies. It reuses the existing outcome rule and locks, with no change to stored
transitions, protocol, schema or observer ordering. It also refreshes the status and stop explanation when rejecting a
cached failure, so a newer recorded result is preserved if it arrives while the reply is being assembled.

The regression seeds both a cached failure and the recorded outcome directly. It covers the ordering that previously
left the wrong status behind, but does not reproduce a live Stop race during a clone. The resolved finding is removed
from the pending queue and a fixed changelog fragment is included.

### Open questions and possible follow-ups

None required for this plan.

### The PRs

- [#2117](https://github.com/scode/farhelm/pull/2117/changes): keep a successfully stopped session from showing a stale
  preparation error; draft based on main.

### Checks run, reused and skipped

- Reused corrected recorder `9442d70a-6c7c-4397-8685-dc42798b0f38`:
  `cargo nextest run -p farhelm-supervisor --lib -E 'test(service::status::tests::)'`, 11 passed, 1,123 unrelated cases
  selected out, no failures, flaky results or runtime skip marker. Manifest, complete JUnit, untruncated output and
  exit-zero result were inspected on resume. Pinned nextest and tmux, four slots, zero retries.
- That run covers source `4c5bc0f1`; the published `e1e7cb56` differs only in PR-link bookkeeping. The resumed head
  `778398f8` preserves the product diff and changes only its queue/report ancestry. Explicit source comparisons
  confirmed both steps. All incoming main changes from `c610827c` through `c3a6b54f` were read: separate helm/trash
  reports and queue state, with no interaction requiring repeated runtime checks.
- Reused corrected `cargo clippy --all-targets -j 4 -- -D warnings` and the prior formatting, changed-Markdown dprint,
  changelog format and isolated delay check. The delay checker inspected 301 delays with no missing reasons. The
  retained Clippy success was inspected; unchanged source and tests preserve these checks' coverage. Explicit-commit
  whitespace validation ran again after rebase and passed.
- Earlier recorder `fae83e2c-7b96-4879-bb3a-9e68a6b26ba4` passed 11 cases before the review correction. It remains
  earlier evidence and is not used as proof of the final implementation.
- Skipped broader Rust, browser, desktop, installer and release tests: focused status-reply cases cover the changed
  projection, and the resume changes no executable code. No UI, installation or native desktop behavior changed.

### The review gate's outcome

The required fresh-context external gpt-6.1-sol high reviewer found a transient inconsistency in the first attempt: a
newer outcome could reject a cached error while the reply retained an older status and stop explanation. That finding
was accepted and corrected. The follow-up found no remaining issues and confirmed the scope and lock lifetimes. Both
full artifacts were read again on resume and checked against the implementation. The reviewer received the full
test-authoring checklist and performed static review only. A fresh external gpt-6.1-sol medium wording read passed; its
full artifact was also read on resume. The documentation pass covers precedence, late cache publication, lock lifetimes
and the regression contract. No unresolved finding or scope departure remains.

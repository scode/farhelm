## What this was about

Cancelling a terminal file download before the host finished starting it left a small request record in the helm until
that host reconnected. Repeating that ordinary action could accumulate records indefinitely. The agreed fix was confined
to downloads; it did not authorize changing how all requests are cancelled.

The download's existing cancellation cleanup now releases its pending request record as well as its transfer route. It
does this without waiting for the supervisor to answer the cancellation, since the supervisor may have stopped the
transfer before it could reply. The original feedback item is removed and its triage outcome records the draft PR.

## Things you should know

The fix follows the planned helm-side approach. Shared request handling, timeouts, quotas and the supervisor protocol
are unchanged. An unrelated request remains pending, and a late reply to a cancelled download is still harmless.

The regression uses a live in-memory protocol connection to hold download startup at the cancellation boundary. It
repeats cancellation three times and checks the actual request records, rather than inferring cleanup from an error or
from closing the connection. It does not measure process memory or reproduce a browser disconnect against a real host.
Existing download HTTP and save tests also passed.

## Open questions and possible follow-ups

None required for this outcome. The new regression proves bookkeeping cleanup; it does not claim a measured reduction in
total process memory.

## The PRs

- [#2197](https://github.com/scode/farhelm/pull/2197/changes): release cancelled download bookkeeping. Draft head
  `85b4d5c0a1962194cf29d481fdbbe72b97b13748`, bookmark `plan/download-cancel-pending-entry/01-retire-entry`.

## Checks run, reused and skipped

The reviewed source was `9ecbc8b6643883e0f26963b4705d96e9a0131a06`. The draft head changes only the commit description
and the triage ledger's PR link from that revision. Both recorder runs captured base `94fede54` plus their retained
working changes; the first predates documentation and bookkeeping edits, while the second covers the complete reviewed
source. The Rust implementation and regression were unchanged between these checks and the draft head.

- Run: recorded nextest selection `cargo nextest run -p farhelm-helm --lib -E 'test(client::downloads::tests::)'`, run
  `a008cd76-9f90-41ea-a19e-a526b25248e0`: 1 passed; 1,118 tests selected out.
- Run: recorded nextest selection `cargo nextest run -p farhelm-helm --lib -E 'test(downloads::)'`, run
  `9634874e-fb65-4996-be3d-f18343a19972`: all 15 selected tests passed; 1,104 tests selected out. Both runs used the
  pinned nextest and tmux, four slots and zero retries. Reports were complete, output reached EOF without truncation,
  and retained output contained no runtime `SKIPPED` messages. These are focused selections, not whole-crate passes.
- Run: `cargo clippy --all-targets -- -D warnings`, `cargo fmt --all -- --check`, changed-file `dprint check`,
  `python3 releasing/check-changelog.py format`, and `git diff --check`: passed. The isolated test-sleep checker
  inspected 302 delays with zero missing rationales. Changed-file dprint and whitespace checks also passed after adding
  the PR link.
- Reused: the successful source checks above for the final ledger-link-only update; no executable behavior changed. Main
  moved only through queue state edits during execution, whose diff was inspected; no source rebase was needed.
- Skipped: browser, desktop runtime, supervisor runtime and the full Rust suite. The changed behavior is isolated helm
  download bookkeeping, covered directly at the unsettled protocol boundary and by the existing download HTTP/save
  tests. No UI, supervisor or protocol implementation changed.

## Review gate's outcome

The required fresh-context gpt-6.1-sol high source review read the complete diff and surrounding cancellation paths and
found no actionable findings. It checked the download-specific complexity gate, cancellation ordering, late replies,
lock lifetimes, the live-connection test premise and documentation. Its review was static; the recorder runs above are
separate execution evidence. The required blind wording review also passed with a fresh reader after an initial reader
invalidated its blind phase by looking at the diff early. No scope expansion or fallback was needed.

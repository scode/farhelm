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

### Landing

Landed in round 76, as #2197, first of the round's two plans and unchanged from what the report describes.

#### What landed since the PR was based, and whether it interacted

The only code that reached main between this PR's base and its landing was the WebKit input-method key guard (#2194),
which changes keyboard handling in the browser interface only. The round's other plan, triage-spec-rules-oct16, changes
only specification text about Claude settings edits, stale views across GUIs and the trash's scale limit, and landed
after this one. I read the WebKit change and the other plan's three spec edits against this PR, and a fresh-context
reviewer did the same independently. Neither found an interaction, and the landing rebase was clean, with the PR
byte-identical to the head the executor delivered.

The reviewer also checked the fix against the helm's request tracking. Request numbers are never reused, so clearing a
cancelled download's entry can never touch a later request. A reply that arrives for an entry already cleared is still
dropped quietly rather than treated as a misbehaving supervisor. SPEC_impl.md accepts leftover request records only when
a misbehaving supervisor never answers; a user cancelling a download is ordinary operation, so that allowance did not
cover this case.

#### Fixes made during the landing

None. Terminal file downloads first shipped in v0.26.1, so the PR's own changelog entry, filed as a fix, is right.

#### Checks

- Run now: none.
- Reused: the download regression run `a008cd76-9f90-41ea-a19e-a526b25248e0` and the wider helm download test run
  `9634874e-fb65-4996-be3d-f18343a19972` (15 passed), plus the report's clippy, formatting, Markdown formatting,
  changelog-format and test-sleep checks. They covered the reviewed source the report names, on the base before #2194.
  Their coverage still applies because the landing rebase left the PR byte-identical, and the only code that joined its
  base, #2194, changes keyboard handling in the browser interface and none of the helm's download or request code.
- Skipped: browser, desktop, supervisor runtime and the full Rust suite, for the reasons the report gives. As the report
  says, the fix is shown by the request records, not by a measured drop in process memory.

Nothing in the executor's report became untrue at landing.

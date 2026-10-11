# Notification bells and read marks under the reload notice

## What this was about

When the window and helm run different Farhelm builds, the sidebar shows a reload notice and stops refreshing
automatically. Closing a session's notification list, clearing its notifications, and marking a session read or unread
still saved on the helm, but the sidebar kept showing the old bell or read state. A second read/unread click could send
the same value again instead of reversing it.

The maintainer chose to fix these three clear bugs together, provided the changes stayed small. All three now request a
refresh after a successful save: closing the notification list quiets its bell, Clear removes it, and mark read/unread
visibly reverses. No outcome was dropped by the complexity gate.

## Things you should know

The fixes use the sidebar's existing explicit refresh. They wait for the helm's answer rather than editing the displayed
state ahead of it. Existing response ordering prevents an older listing from restoring stale marks after the refresh.
The read/unread save can outlive the sidebar, so its success asks the still-mounted sidebar to refresh; a late result
after the sidebar disappears is safely ignored.

Failure behavior is preserved: failed notification reads remain silent and retry on the next close; failed Clear and
manual read/unread saves still show a row error. No reader or write-queue contract changed. The draft also records the
three completed triage outcomes, removes their feedback entries, and adds a release-note fragment.

Validation exposed an existing test setup error: the failed-save row was above another row's open menu, so adding its
error line moved that menu and legitimately closed it. The same case failed against the retained pre-fix UI in both
engines. Its fixture now establishes and checks the other row above the failed row, excluding layout dismissal while
preserving the original error, visibility and focus assertions. Product behavior was unchanged by that correction.

## Open questions and possible follow-ups

None. The work stayed within the agreed scope and needs no product decision.

## PRs

- [#1907](https://github.com/scode/farhelm/pull/1907/changes) — all three fixes in one draft PR and commit. The executor
  has neither marked it ready nor merged it.

## Checks run, reused and skipped

Ran `cargo fmt --all -- --check`, `cargo clippy -p farhelm-ui --all-targets -- -D warnings`, the source delay checker
with its isolated pinned parser environment, the changelog format check, and `dprint check` on the changed applicable
Markdown. All passed; the delay checker inspected 287 delays with zero missing reasons. The configured dprint plugins do
not format TypeScript.

The recorded focused Rust test `cargo nextest run -p farhelm-ui -E 'test(seen_toggle_report)'` passed: one test, four
global slots, zero retries, run `994cfad7-4a04-4954-8d57-96756b747bca`. It proves success requests a refresh, failure
does not, and both late outcomes tolerate an unmounted sidebar.

Built the CLI/supervisor and the release web UI. The complete notification spec passed 16 cases, eight per engine, run
`f33e4025-e87d-4a3b-95ea-285186a22bb8`. The focused sidebar manual read, menu and status-dot selection ran 12 cases: 10
passed, including the new mismatch toggle on both engines, and the existing setup error above failed on both, run
`64abda56-6d4f-4a0d-87f3-1f227ee7e2a5`. After its fixture correction, that exact case passed on both engines, run
`8372a3b1-b9fa-422e-96fd-a7cd1bcbf578`. The unchanged 10 successful cases were reused. Each recorded selection uses
Chromium and WebKit, one browser worker, zero retries, and the pinned tmux 3.7c. They need only the connected local
supervisor; the fixture's unavailable SSH host is outside their coverage.

The exact existing-case comparison against retained pre-fix web assets failed twice at the same assertion, run
`36d199ec-a254-455f-b791-342ff2c56397`. Source metadata still contained the fix; the generated web entrypoint selected
the retained pre-fix hashed JavaScript and Wasm. The fixed entrypoint was restored before final validation. An initial
anchored grep selected no tests, run `f693dd81-19fc-4b18-b0b6-f5d7726fd5f4`; it provides no runtime assertion evidence.
These failed records are preserved privately with their disposition.

Before applying the fix, all three new regressions failed at the intended stale-state assertions on both engines: six
executed failures, zero skips or unstarted cases, run `2e034fa4-ff9a-4040-951d-124a2f6e5194`. That deliberate baseline
remains retained as evidence; it is not a latent flake.

The native checks, builds and notification run covered the implementation based on `d2410be4`. Their evidence is reused
after the rebase: intervening main changes only moved plan claims and delivered another plan's report, with no product,
specification or test changes. The final sidebar runs covered the same production code based on `c4c99ec0`; only the
existing test fixture changed before its exact rerun. No runtime check is repeated for bookkeeping-only PR links. Full
workspace and browser suites, desktop smoke, asset parity, installer, provisioning and website checks were skipped
because the affected refresh and lifecycle contracts are covered by the focused checks and none of those components
changed.

## Review gate's outcome

The required fresh-context gpt-6.1-sol reviewer at high effort found no correctness, design or idiomaticity issues. The
review included the full test-authoring contract, response ordering, unmount safety, unchanged failure paths, and all
six baseline failures. A bounded follow-up reviewed and accepted the existing test's fixture correction and its passing
evidence. The executor inspected the complete findings and checked them against the source and evidence. Commit and PR
wording passed a separate fresh-context Sol medium cold read after one wording revision.

Implementation and investigation were local, as required by no-workhorse mode. Native model attribution and usage
counters are unavailable; this evidence gap is retained in private galaxy-brain session
`5c9810ab-dcb4-4a62-9a79-bfb239f2d810`, with its location in the plan working log beside the checkouts, and does not
affect the validation results.

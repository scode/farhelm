### What this was about

In the trash dialog, a host with archives Farhelm could not inspect could read “0 checkouts · 0 B.” A mixed listing
could show its measured size without saying that other archives were unmeasured. The diagnostics were visible and
cleanup already handled those entries, but the host's summary implied a complete count and size.

The approved fix was to qualify each host's summary using the existing “unchecked entries” wording and partial-size
mark. A shared calculation now supplies the host summary and its frozen cleanup confirmation, within the agreed small
display/helper change.

### Things you should know

An issue-only host now reads “0 checkouts · 1 unchecked entry · 0 B measured · partial” for one distinct unchecked
entry. Mixed listings retain the known byte sum and mark it partial. Multiple diagnostics for the same entry count once.
Listings with fully measured checkouts and no unchecked entries keep their previous count and size wording.

This changes the host summary only. Global totals and cleanup confirmations already disclosed unchecked entries; their
displayed behavior stays the same. Connection checks, cleanup limits, frozen selections and host-side deletion authority
are unchanged. The resolved finding is removed from the pending queue and a fixed changelog fragment is included.

### Open questions and possible follow-ups

None required for this plan.

### The PRs

- [#2118](https://github.com/scode/farhelm/pull/2118/changes): qualify each host's trash totals when archives could not
  be checked; draft based on main.

### Checks run, reused and skipped

- Recorded `cargo nextest run -p farhelm-ui --lib -E 'test(list::trash::tests::)'`: three passed, 498 unrelated tests
  selected out, zero failures or flaky results. Recorder `95921264-9689-46c3-9ca5-b2b3e6ec6067`, four slots, zero
  retries, pinned nextest and tmux. The complete manifest and JUnit agree with the exit-zero result; retained output has
  EOF, no truncation and no runtime skip marker.
- The new table case covers issue-only and mixed listings, duplicate diagnostics, known and unknown sizes, and the
  matching frozen selection. Existing single-entry authority and selection-freezing tests also passed.
- `cargo clippy -p farhelm-ui --all-targets -- -D warnings` passed. Cargo formatting, changed-Markdown dprint, changelog
  format, explicit-commit whitespace validation and the isolated delay checker passed; 301 delays inspected, zero
  missing reasons.
- No previous runtime results were needed. Tests cover the implementation in `7c93d022`; the later PR-link update does
  not change source or tests. Incoming main through `a7dbb686` changes only another plan's claim state, with no source
  interaction requiring rebase or repeated runtime checks.
- Skipped broader Rust and browser suites, desktop, installer and release tests. The changed Rust directly supplies the
  host header's text; no CSS, browser event handling, network request or deletion flow changed. Focused summary and
  selection regressions cover the concrete risk. No visual or live filesystem-inspection smoke was performed.

### The review gate's outcome

The required fresh-context external gpt-6.1-sol high reviewer found no issues and confirmed the complexity gate. The
full artifact was read and checked against the implementation and relevant specifications. It was static review;
validation was performed separately by the executor. The reviewer received the full outcome and test-authoring
checklist. A separate gpt-6.1-sol medium wording cold read passed. The documentation pass covers evidence versus
deletion authority, distinct diagnostic counting and the regression's purpose. No unresolved finding remains.

### Landing

Landed as #2118, the sixth of the round's seven plans, tested stacked with the others on main.

#### Review

An independent fresh-context review found nothing blocking: every mix of checked, unchecked and unsized entries reads
accurately, unavailable hosts are unchanged, the delete confirmation stays consistent with the permanent-loss warning
from #2080, and no browser test pinned the old totals.

#### Fixes made while landing

The triage ledger's Execution field gained its "complete" state and a linked PR, like the other entries, and the
changelog fragment now reads: each host's summary counts the archives Farhelm could not check and marks its size as
partial.

#### Left as is

Possible follow-ups outside the plan's limits: the global header total does not reuse the new shared sum, so on a host
over the 10,000-entry limit unchecked entries would not mark it partial; the trash button's tooltip says "empty" when
the only entries are unchecked ones.

#### Checks

Run on all seven of this round's plans stacked on main (managed-checkout-row-accessibility, download-long-name-copies,
save-template-ime-escape, helm-start-without-ca-bundle, stopped-session-launch-error, trash-totals-unchecked,
checkout-override-error-wording), with every landing fix in:

- `cargo fmt --all -- --check`, `dprint check`, the changelog lint and the sleep checker (301 delays, none missing a
  reason); `cargo clippy --all-targets -- -D warnings`, `cargo clippy -p farhelm --bins -- -D warnings`, the UI crate's
  clippy with the desktop feature and `cargo check -p farhelm-desktop`.
- Through the recorder, after rebuilding the CLI: the protocol, helm and supervisor packages in full with pinned tmux,
  run `4256456b-4340-4e64-86a5-85aa71206a7a`: 2395 passed; the UI crate with the desktop feature, run
  `5d0c7c3a-4d29-4a92-a8e2-a1eba6893786`: 656 passed.
- The JS harness (257 passed) and the website build.
- The github-checkouts, templates, trash-dialog, sidebar, provisioning and tooltip-coverage specs on Chromium and
  WebKit, run `7f4cef76-eea8-4b05-9a31-1260bb14498a`: 456 passed.
- Not run: the CLI's end-to-end targets and other browser specs; no PR in this round changes them.

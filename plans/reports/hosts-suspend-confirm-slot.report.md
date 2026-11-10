## What this was about

In Hosts, answering an old Suspend question could still pause a sandbox after Cancel. An old Cancel, Close or Escape
could also close a newer question. The existing confirmation rule requires each answer to act only on the question it
was drawn for, as host Remove already does. This plan applies that rule to the Hosts-panel Suspend question.

## Things you should know

Cancel, Close, Escape and Suspend now consume only their own question's opening. An answer from a cancelled, replaced or
already-answered question does nothing, including leaving a newer question and its focus isolation alone. A current
Suspend answer still closes the question and starts the same request; it holds no new operation lock. The question still
opens only after the existing activity check needs confirmation.

The session toolbar's separate Suspend question is outside this plan. The fix, helper regression and explanatory
comments total 80 added/deleted lines under the size gate, excluding outcome bookkeeping and the changelog fragment. The
regression covers cancellation, reopening for the same host, replacement by another host and duplicate answers.

## Open questions and possible follow-ups

No new follow-up identified. The separately queued session-toolbar question remains unchanged.

## The PRs

[PR #3299](https://github.com/scode/farhelm/pull/3299/changes) — ignore stale host Suspend answers; draft head
`368d33c2ba8421651a1f79bd061ec983ba6fd7e4`.

## Checks run, reused and skipped

Run during implementation:

- Recorded `cargo nextest run -p farhelm-ui --lib -E 'test(/^hosts::/) | test(/^ops::/)'`: 114 passed, including the new
  Suspend regression; 824 tests outside the filter were not run. Run `5dadc8b1-364b-48dc-9554-a19e74c17818` used
  controlled nextest 0.9.148, four global slots, zero retries and tmux mode `none`. The source identity, JUnit report
  and captured output are complete, with no runtime `SKIPPED` lines, omitted output or truncation. This selection covers
  the changed question handlers and the shared confirmation mechanism.
- The first recorded attempt, `6b1cefed-086b-495d-afa7-3186ddae88f6`, failed compilation before executing tests: Dioxus
  rejected a local binding inside the rendered conditional. Moving the binding before the render fixed it. That failed
  record is retained privately; it is not counted as a test pass or a latent flake.
- `cargo fmt --all -- --check` and `cargo clippy -p farhelm-ui --all-targets -- -D warnings` passed, checking the
  changed Rust source and every UI target.
- `python -B scripts/check-test-sleeps.py` with the isolated parser interpreter passed: 429 delays checked, zero
  unannotated. This checks the changed test against the repository's delay rules.
- `python3 releasing/check-changelog.py format` passed for 38 fragments. Targeted `dprint check` and whitespace checks
  passed for the outcome graduation and queue index.
- After the rebase, `python3 scripts/check-agents-md.py` passed with zero errors and one existing archive-citation
  warning, checking references affected by removing the feedback file.

Reused for the delivered head: the successful UI tests, formatting, Clippy, sleep check and code review above covered
base `b29bdbf8` plus the final working diff (fingerprint
`47f14fc4a9c0efda2821620d9e59a768af57147de21a2de2ef1f0d95fca99138`). The rebase onto `37b6fe23` added an unrelated
supervisor repository-cache fix and planning bookkeeping. Its conflicts were confined to shared outcome bookkeeping and
were resolved by preserving both outcomes. A direct comparison confirms the tested UI source and changelog fragment are
unchanged at the delivered head. Later main changes through `fc454545` are planning bookkeeping only.

Skipped: browser tests, because the existing ordinary Suspend test requires a new web build and stack but does not
exercise queued stale answers; the focused handler regression covers that distinction directly, and no dialog markup,
focus script or timing mechanism changed. Full workspace, desktop and cloud tests add no identified coverage for this
bounded UI change. Hosted CI was not dispatched; it runs only on demand. The live installation was not used.

## Review gate outcome

The required single fresh-context GPT-6.1 Sol reviewer at high effort passed with no actionable findings. It received
the full plan and the test-authoring checklist. No review findings were rejected. A separate blind commit-message reader
understood the cancellation and replacement problem and found the wording consistent with the change.

### Landing

Landed alone (the only plan in this landing round), as #3299. Since these changes were written, main gained user tmux
jobs surviving scoped cleanup (#3297), the remote Mac `stat` fix (#3298) and a TODO entry (#3301); none touches the
Hosts panel. Rebasing conflicted only in the triage records, where both sides had moved entries; the landing kept both.
The landing made no other change.

#### Review

The landing's fresh-context review found nothing that must change. It traced every way into and out of the Suspend
question: opening it again for the same or another host, Escape, the close button, Cancel, Suspend, the host
disappearing, the panel closing, the question waiting behind another dialog, and the suspend request being refused after
Suspend. None leaves the page locked behind a dialog that is gone, or a dialog whose buttons do nothing. An answer from
a question that was already answered or replaced now does nothing, and the newer question keeps working. The change
follows host Remove's pattern, and the session top bar's own Suspend question, a separate undecided review item, is
untouched.

#### Things you should know

- The evidence splits in two. Rejecting a stale answer is covered only by the new unit test, which checks the helpers
  that accept or refuse an answer; no test sends a stale answer through the real panel. That test cannot see whether the
  panel hands the dialog the right question: if it did not, every answer would silently do nothing and the test would
  still pass. The browser runs under Checks cover that wiring: they drive the real Cancel, Escape, close and Suspend
  paths and passed. The executor skipped browser tests as adding nothing; the landing ran them for this gap. No test
  checks that focus returns to the host's row after a dismissal.
- The race fixed here only exists within one redraw: a Cancel and a Suspend handled before the page redraws. SPEC.md
  already accepts that this narrow race can happen with any dialog; the new changelog entry does not say how narrow it
  is, which release curation may want to.
- The held plan that stops pending actions from locking the page (nonblocking-session-actions) has open PRs that touch
  SPEC_impl.md's rules for confirmation questions but not the Hosts panel; they should rebase over this cleanly.

#### Checks

As the maintainer asked for this session's landings, the browser specs ran on Tensorlake sandboxes (cloud test
machines), and everything else ran on this development machine, on the tree that merged.

- Here: `cargo fmt --all -- --check`, `python -B scripts/check-test-sleeps.py`, `dprint check`,
  `python3 releasing/check-changelog.py format`, `cargo build`, the release web build
  (`dx build --package farhelm-ui --platform web --release`), `cargo clippy -p farhelm-ui --all-targets -- -D warnings`
  and `cargo check -p farhelm-ui --features desktop` passed.
- Here, recorded `cargo nextest run -p farhelm-ui`: 938 passed, none skipped (run
  `4dc159a0-7a08-4a75-939b-77bf08d31952`).
- Tensorlake, the browser specs that show the Hosts panel's Suspend question, on Chromium and WebKit on ten sandboxes,
  using the release web build (`scripts/tensorlake/sandbox-tests.ts run --template <template> --shards 10` with the
  Tensorlake hosts, Tensorlake lifecycle, sandbox sessions, Tensorlake settings and sounds projects of both engines):
  114 browser tests passed, none failed (run id prefixes `ddf851d2`, `108927af`, `7734c893`, `60e7afe9`, `97829339`,
  `bf279653`, `b66e361e`, `86106095`, `d7471282`, `31ecbfc2`). The lifecycle spec's tests dismiss the Suspend question
  by Cancel, Escape and the close button, reopen it, and suspend through it, which is the real path the new unit test
  cannot reach. The runs went through the recorder's general command mode, because its Playwright mode always runs both
  engines and these runs split the engines across sandboxes. The sandbox image runs Node 24, a slightly older release
  than this machine's. Every sandbox reported that the services the tests rely on (a per-user systemd and SSH to itself)
  worked.

Reused from the executor: nothing; the runs above include its UI crate tests.

Skipped: the rest of the browser suite (the change is the Hosts panel's Suspend question alone, which the specs above
drive), the workspace's other Rust tests (only the UI crate changed), and hosted CI (manual-only, not dispatched).

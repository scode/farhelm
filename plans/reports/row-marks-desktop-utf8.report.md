## What this was about

In the desktop app, a session notification bell could put stray characters over the agent and permission marks beside
it. This looked like overlapping marks, but the real desktop window confirmed a decoding error: the stylesheet's
invisible character was being read as visible `â€‹`. The desktop page and its text assets now declare UTF-8, so the
bell's baseline stays invisible and the surrounding marks stay readable.

## Things you should know

The invisible character and row layout are kept. Every desktop text asset now declares its encoding, including
JavaScript; this also corrects decoding for the terminal library's non-ASCII text. Binary fonts keep their existing
content types and bytes. The web UI already declares UTF-8 and needs no change.

The before/after proof used the existing desktop smoke's real WebKitGTK window on Linux. It did not run a Mac window. A
temporary ASCII-only diagnostic reported the document encoding and the loaded bell rule's characters; it was removed
before the final production checks and commit. Those readings are prior executor observations recorded in the working
log. Recovery found that the raw window diagnostic logs had not been retained; the recorder records retain the complete
smoke verdicts, not those readings.

## Open questions and possible follow-ups

No decision or follow-up is required by this plan. A manual Mac visual check can confirm the original reported
appearance on that platform; the automated proof directly reproduced and corrected the decoding mechanism in the desktop
engine family.

## The PRs

- [Draft PR #1724](https://github.com/scode/farhelm/pull/1724/changes) — desktop UTF-8 declarations, response-contract
  tests, implementation specification and removal of the covered TODO entry, in one draft PR.

## Checks run, reused and skipped

Ran Rust formatting, changed-file dprint, changelog-format lint, the isolated source-delay checker (269 delays, zero
missing reasons), workspace all-target Clippy, desktop-feature UI all-target Clippy, desktop UI compilation and shipped
desktop binary compilation. The additional desktop-feature lint covers the changed native modules that the ordinary
workspace lint omits. A separate documentation pass covered every touched file.

Recorded runtime evidence:

- Baseline desktop smoke `09bc082d-55d0-4809-946c-54f8bf91e86f`: complete PASS and cleanup. The real window reported
  Windows-1252 and visible code points corresponding to `â€‹` in the bell rule, reproducing the planned cause even
  though the existing integration smoke passed.
- Corrected desktop smoke `44cf667e-ed17-42d6-affa-027c407b5276`: complete PASS and cleanup. The real window reported
  UTF-8 and the intended zero-width character. Both runs served all 20 requested assets without misses and passed the
  restart/authentication/managed-supervisor gates. These measurements are reused after removing only the ASCII
  diagnostic script; production encoding declarations and served bytes are unchanged.
- Desktop Rust target run `d05411e9-d8d2-4105-859e-4db93fc6ec66`: 540 passed, zero failed, skipped or flaky; complete
  JUnit and output, recorder cleanup complete. Includes the dedicated stylesheet charset/unchanged-byte test and the
  binary-font content-type test.

Validation covers the five-file working diff on base `5c47621b`. Main advanced to `1f237fc8` with only four queue-claim
transitions; every intervening diff was inspected and no product behavior or contract changed. Reuse is valid after
rebasing onto that main, with no further runtime run warranted.

Skipped full workspace runtime, browser, installer, provisioning and website batteries: the change is confined to native
page/asset encoding, covered by the desktop response tests, compilation and actual-window before/after smoke. There are
no executable documentation examples, new asset paths, or web behavior changes. No hosted CI, deployment or publication
was requested.

## Review gate's outcome

The sole required code reviewer ran on gpt-6.1-sol at high effort with the full plan charter and verbatim test-authoring
checklist, and found no defects. The executor read the complete artifact and checked the implementation against the
measured failure. Runtime observations were independently made by the executor; the reviewer did not repeat them. Commit
and PR-title wording passed a separate fresh gpt-6.1-sol medium cold read after one plain-language rewrite. The PR body
is empty. Implementation and investigation stayed local; native model-reporting and usage counters were unavailable, and
private galaxy session evidence records that gap.

### Landing

Landed on 2026-10-09 (UTC) as #1724 (the desktop app declares UTF-8 for its page and text assets), one squash commit on
main.

#### What else was on main

Between the commit the change was built on and the landing, main gained the save-as-template change to the launcher
(#1717), the repository-cache change landed just before this one (#1720), documentation and TODO entries, and the
planning queue's bookkeeping. None of it touches the desktop app's asset handling. The only conflict on the way was in
TODO.md, where #1720 and this change removed neighbouring entries; both are gone.

#### Review before merging

A separate reviewer that had not worked on the plan read it against main and the other two plans in the same round
before anything merged and found nothing that breaks. The desktop asset check compares only which files are bundled, and
the desktop smoke test does not check content types, so neither is affected.

#### Checks

- Run now, with this change stacked between #1720 and the conversation-warning plan: the compile, lint and format checks
  listed in #1720's landing notes, all clean, including `cargo check -p farhelm-ui --features desktop`; and the desktop
  UI's unit tests through the test-run recorder (run `b9f0a31b`, 545 passed).
- Reused: the report's checks, including its before-and-after desktop smoke runs.
- Skipped: a Mac window, which still needs a Mac.

Nothing in the report above was made untrue by the landing.

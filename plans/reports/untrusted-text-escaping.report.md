## What this was about

A host could send hidden or direction-changing characters that disguised a session's status diagnostic or sidebar
folder. A malformed message or refused terminal attachment could also send terminal controls into the helm's logs. The
maintainer accepted these four triage findings as straightforward fixes and authorized one shared PR.

## Things you should know

All four fixes use the existing escaping behavior. Ordinary status wording and folder abbreviation stay the same. Empty
sidebar folders stay blank: the triage ledger explicitly required that, overriding the planner's proposal to show
`(empty)`. Empty or whitespace-only status diagnostics now have a visible placeholder, as the plan intended.

No new helper, formatter, protocol, or log-capture machinery was needed. No outcome tripped its complexity gate. The
four feedback entries were removed and their execution records link the shared PR.

## Open questions and possible follow-ups

None. The raw host name beside the folder was explicitly outside these findings. This work does not claim a broader
audit of all host text.

## PRs

- [#1797: fix: escape host text in session details and logs](https://github.com/scode/farhelm/pull/1797/changes) — one
  commit covering all four outcomes; left as a draft for the plans monitor.

## Checks run, reused and skipped

- Ran focused nextest through the recorder for the status tests, sidebar row tests, and helm's existing
  text-normalization test: 37 passed, 1,425 selected out, no failures or runtime early-return skips. Recorder run
  `545b2eb3-17ff-408c-abb4-5a648fffff1e`; pinned nextest 0.9.143 and tmux, four slots, zero retries. The two changed GUI
  regressions passed.
- Ran `cargo clippy -p farhelm-ui -p farhelm-helm --all-targets -- -D warnings`, `cargo fmt --all -- --check`,
  `dprint check TRIAGE_OUTCOMES.md review_feedback_queue/INDEX.md`, and `python3 releasing/check-changelog.py format`:
  passed.
- Ran `python -B scripts/check-test-sleeps.py` with the isolated pinned interpreter: 281 delays inspected, zero missing
  rationales.
- Reused the successful runtime and source-review evidence after rebasing from `c229b860` onto `ee306896`: all upstream
  changes were queue claims, with no source or spec interaction. The later test-helper docstring, feedback-index
  correction, and PR-link amendment do not alter tested behavior.
- Skipped browser and broader workspace runtime tests because the changes only route text through existing helpers. The
  sidebar regression observes actual rendered text mutations; pixel layout, clipping, and emitted log output were not
  tested. The two log sites have no new capture test, per the plan's scope; the existing escaping-and-bounding helper
  test passed.

## Review gate outcome

The required fresh source review, requested on gpt-6.1-sol at high effort, found no remaining correctness, design,
idiomaticity, or test-quality issues. It inspected all four sites, their helper contracts, the GUI tests, and
bookkeeping. A wording cold read passed. An incidental finding about wrapped feedback-index continuations was fixed and
the source reviewer verified the correction.

The reviewer performed source inspection; the executor verified the runtime and lint results. Native runtime model
identity and token counters were unavailable. The executor did not mark the PR ready or merge it.

### Landing

Landed on 2026-10-10 (UTC) as #1797. This round landed five triage plans together, in order: untrusted-text-escaping,
git-env-isolation, os-readback-fixes, harness-tooling-fixes and ssh-config-atomic. Each rebased onto main with only
conflicts in the review queue's index, where each plan removes only its own entries. Since their stacks were based, main
gained this day's earlier landings (sounds, file downloads, the reboot follow-up of the supervisor's timer sweep) and
the 2026-10-10 spec triage; of the files these plans touch, only the helm's supervisor client changed upstream (download
routing), away from the log line one of them changes. A separate reviewer read all five against each other and main by
reading the code only, and checked each against its triage decisions and completion criteria.

#### A fix made while landing

The triage outcome asked for a test of the escaped "invalid frame from supervisor" log line where a practical seam
exists. The executor skipped it, believing the helm had no way to capture its own logs in a test; it does, and an
existing client test already uses it. The landing added that test: a fake supervisor sends a message whose name carries
an escape sequence and a bell, and the logged error must show them escaped while still naming what was sent. It fails
with the escaping removed. In #1797 before it merged.

Smaller notes left as they are: the empty-folder test only checks that no "(empty)" text appears, not that the line is
blank; and an exited session whose annotation is empty text now shows "— (empty)", which the outcome allows.

#### Checks

- Run now, on the first four stacked in landing order: `dist generate --check` (the release workflow matches its
  sources), `cargo clippy --all-targets -- -D warnings`, `cargo clippy -p farhelm --bins -- -D warnings`, the
  supervisor, helm, UI and protocol unit tests in full through the recorder with pinned tmux 3.7c, four slots and no
  retries (run `1b086587`, 2699 of 2699), and on Chromium and WebKit with one worker and no retries the spawn, header,
  readers and change-feed specs (run `50ffb9fe`, 44 passed; the two skipped are the real-Claude spawn cases).
- Run now, after the landing's fixes, with ssh-config-atomic stacked on top:
  `cargo clippy -p farhelm-helm --all-targets`, `shellcheck` on the provisioning script, and the helm's client tests
  including the new log-escaping test (run `0067a921`, 68 of 68). The new test was also seen to fail with the escaping
  removed, then restored.
- Reused from the executors: their focused runs for each fix, the hosted macOS compile of the argument-reading change,
  and the deflake end-to-end evaluation.

Nothing in the report above was made untrue by the landing.

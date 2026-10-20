# Cancelled transfer bookkeeping

## What this was about

Two narrow cancellation windows could retain bookkeeping in the helm until its host connection ended. A download could
finish receiving bytes, then lose its cleanup when cancelled while waiting for the routing lock. An upload cancelled
before its host answered admission could leave that unanswered request registered. Repeated cancellations could
accumulate those entries. The maintainer chose to fix each only within roughly 15 lines of non-test code.

The download keeps cancellation cleanup armed until route removal finishes. The upload guard owns its admission request
as well as its route and retires both before waiting to send Abort.

## Things you should know

Both fixes met their size gates: four added/removed non-test lines for the download and ten for the upload, including
their explanatory comments. Tests are excluded from those agreed gates. Neither required a supervisor or protocol
change. File contents, completed transfer results and cancellation messages are unchanged; these are internal cleanup
fixes. Both fragments use `kind: none` for that reason.

The upload regression uses a live scripted peer that withholds its admission reply. It also fills the helm's outbound
writer queue to prove local cleanup does not depend on Abort delivery, preserves an unrelated pending request, and
checks a late admission reply. It does not reproduce a real supervisor's saturated reply queue.

## Open questions and possible follow-ups

None required for this plan. The work remains a linear stack of draft PRs for the landing monitor. The live installation
was not changed.

## The PRs

- [#2395](https://github.com/scode/farhelm/pull/2395/changes): release completed-download routing after cancellation.
- [#2400](https://github.com/scode/farhelm/pull/2400/changes): release cancelled-upload admission bookkeeping, based on
  #2395.

Both feedback files and index entries are removed. Their outcome Execution fields name the stable change IDs, bookmarks
and PRs.

## Checks run, reused and skipped

- `cargo nextest run -p farhelm-helm --lib -E 'test(client::downloads::)'` through the recorder with pinned nextest,
  four slots, zero retries and no tmux: 2 passed, 1,208 selected out. Run `a86876bc-579d-4408-97a4-6711eeb5ecd7`
  captured base `6bfbe90895754663707b744f0a3aa3b6c149f036` plus the reviewed working diff, fingerprint
  `5f1e0fd1912dc0e8e906a5b6cd9d9b29eb74106d48c066e98be97ae2b2ecbf1b`. Both the new contended-EOF regression and the
  existing admission cancellation regression executed. Report and output are complete, with EOF, no omitted bytes or
  cleanup limit, and no runtime `SKIPPED` marker.
- `cargo nextest run -p farhelm-helm --lib -E 'test(client::downloads::) | (test(client::tests::) & test(upload))'`
  through the recorder with pinned nextest 0.9.148, four slots, zero retries and no tmux: 18 passed, 1,193 selected out.
  Run `4e4ce991-ae32-49f7-8c11-7c628c8ab8de` captured #2395 at `9db65ab16daf0c4ba51dd46e1507e759156b857f` plus the
  reviewed upload working diff, fingerprint `2ad8f74c54b3e62da36ab178f167bca58151628090e30c8aed9584673772429e`. Both new
  regressions and the existing upload admission, refusal, commit, credit and abort cases executed. Complete report and
  output, EOF, zero omitted bytes, no cleanup limitation or runtime `SKIPPED` marker. The tests use in-process scripted
  peers and need neither tmux nor a system service manager.
- `cargo fmt --all -- --check`, `cargo clippy --all-targets -- -D warnings`, dprint for the changed Markdown,
  `python3 releasing/check-changelog.py format` (17 fragments), the isolated test-sleep checker (304 delays, zero
  missing rationales), and `python3 scripts/check-agents-md.py` (zero errors or warnings): passed. These cover Rust
  diagnostics, formatting, fragment shape and the changed tests' source rules.
- Reused the runtime and Clippy results after commit creation and ledger URL amendments: source behavior and tests are
  unchanged. Main through `3a8c976c602c62685a74a9a5752aef28af092026` adds only plans, queue/report bookkeeping and TODO
  references relative to the claimed base; no runtime code or specification contract changed. No rebase or extra runtime
  check was needed for those additions.

Skipped the workspace battery, browser and desktop runtime, installer and release checks: these changes affect only
client-side cleanup ownership. The selected scripted-peer tests cover the changed cancellation boundaries and existing
upload lifecycle behavior. No GUI, filesystem operation, supervisor implementation or wire shape changed. No CI run is
implied by draft publication; repository CI is manual.

## The review gate's outcome

Each PR passed its single fresh-context gpt-6.1-sol reviewer at high effort, with no findings. Commit wording passed
independent cold reads. The required documentation pass covered both guard changes, their regressions and helpers, the
outcome ledger, fragments and feedback index.

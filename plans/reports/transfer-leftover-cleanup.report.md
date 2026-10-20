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

### Landing

Landed as #2395 and #2400, merged bottom-up, with the same content the report describes, rebased onto this round's main
(see the end of these notes). It was the fourth of sixteen plans in this landing round, after launcher-retry-identity,
launcher-composer-drafts and terminal-link-and-copy-fixes.

#### What landed since the stack was based, and whether it interacted

Since the stack's base, main gained round 107's four plans, the three plans earlier in this round, and documentation and
plan bookkeeping. A fresh-context reviewer and I read them and the other plans of this round against this plan. Two
touch the helm's transfer handling or its shutdown:

- host-entry-session-ids changed how the helm identifies sessions. The helm's upload and download handlers now resolve
  the browser's full session reference and send the host only the host's own session ID, which is the ID this plan's new
  tests send, so the fixes and tests agree with it.
- helm-lock-outlives-work keeps the helm's state-folder lock until the helm has finished shutting down. The cleanup this
  plan adds stops when the helm shuts down and never touches the state-folder lock, so the two do not interact.

A rule added to SPEC_impl.md during this round prefers cleaning up on quit over sweeping leftovers at the next start.
This plan does neither: it releases in-memory bookkeeping in the running helm when a transfer is cancelled. The review
also checked this plan on its own terms: the upload fix now forgets the transfer before telling the host to abort, and a
late message from the host for that transfer is then ignored rather than treated as an error. The only conflicts were in
the review feedback queue's index, where each plan removes its own entries; all removals are kept.

#### Fixes made during the landing

None.

#### Checks

Every plan in this round was executed on a main from before round 107's session-identity change, so none had been tested
against it. That is why this round ran the whole Rust workspace and the full browser suite rather than a selection. All
of the following ran now, on all sixteen plans of this round stacked together on main, including the fixes made while
landing plans in this round:

- `cargo fmt --all -- --check`, `cargo clippy --all-targets -- -D warnings`,
  `cargo clippy -p farhelm --bins -- -D warnings`, `cargo check -p farhelm-ui --features desktop`, the isolated
  test-sleep check, `dprint check` on every changed Markdown, JSON and TOML file,
  `python3 releasing/check-changelog.py format`, `python3 scripts/check-agents-md.py` and its `--self-test`,
  `python3 releasing/notes-review-page.py --self-test`, `scripts/check-desktop-assets.sh`, and the website build
  (`bun run build` in `website/`): all passed.
- `cargo nextest run --workspace --exclude farhelm-desktop` through the test recorder with pinned nextest and tmux, four
  parallel test slots and no retries, run `40f2b98b-b160-4239-a653-44cd0bcdcf96`: 3948 passed.
  `cargo nextest run -p farhelm-ui --features desktop`, run `d288b68b-249f-4f21-9fe0-d4d6d1c4a738`: 717 passed.
  `node --test` in `crates/farhelm-ui/js-tests`, run `f82f83f8-67c9-4e07-99ed-467c9830dc9f`: 317 passed.
- The full browser suite (`npx playwright test` in `e2e/`) on Chromium and WebKit, one worker, no retries, run
  `81d76a96-912f-47ed-8e1f-57423edbeb32` (kept): 2183 passed and 14 skipped. Of the skips, 9 are the WebKit clipboard
  cases, because Playwright cannot grant WebKit clipboard permissions; 4 run only against a real, signed-in agent; and 1
  is a pixel capture that runs only on Chromium. One test failed: on WebKit, "model-only number key opens New with a
  temporary composition explanation" in `templates.spec.ts` timed out at its first step, waiting 60 seconds for the
  header's Templates button to settle before clicking it. No plan in this round changes that button or that test, and
  the file's other WebKit tests that open the same dialog passed.
  `npx playwright test --project=webkit-templates -g 'model-only number key opens New with a temporary composition explanation' --repeat-each=20`
  then passed all 20 (run `6c382916-77a1-4ecb-b4ee-a1dae8cac848`). The cause is probably machine load during the full
  run, not something this round broke, but that is not confirmed; on one observation it is not recorded as a flake.
- Skipped: the native Mac app (not runnable here) and the installer (unchanged).

The report was written on a main from before round 107's session-identity change, so its statement that no rebase was
needed and its focused checks no longer describe what landed; the combined runs above supersede them. Nothing else in
the executor's report became untrue at landing.

## What this was about

Six accepted findings concerned maintainer tooling. Two were separate defects in tests that probe how the supervisor
handles child processes: each test could write outside its fixture directory when a temporary path contained spaces. The
real-agent browser test could unregister another run's jj workspace, leaving that run's checkout outside its
repository's workspace tracking. Deflake, the automated flaky-test sweep, could terminate an unrelated process by
trusting a daemon PID left by an old run. Screen captures destined for public test fixtures could retain a private
hostname domain. Failed-release advice told the operator to reuse a spent tag.

The maintainer authorized small fixes together in one draft PR, with any substantial expansion left out for a decision.
All six outcomes were implemented within that scope.

## Things you should know

The probe fixtures quote their paths, and the real-agent test uses its existing run stamp for its workspace name.
Deflake records both the daemon PID and its start time at both publication sites; liveness and stop refuse missing,
malformed, legacy, or mismatched records. A small risk remains: the daemon can exit and its PID can be reused after the
identity check but before the signal, or a replacement process can have the same reported start timestamp. These
short-window risks are an existing documented acceptance from 2026-10-05, not a new decision made by this executor. This
work introduces no new daemon protocol or process-identity library.

Use the updated driver between sweeps. An old-format record is deliberately treated as not running, even if an older
daemon is actually still alive: the new status/stop commands cannot manage that daemon through its bare PID, and wait
can report it as dead. Finish the earlier sweep before switching drivers; a refusal is not proof that its process
exited. The daemon's existing machine-wide lock still prevents a new sweep from running alongside it. There is no
legacy-record migration or fallback signal.

Hostname scrubbing handles the full qualified name before its short label, including case variants. Review caught the
case-variant leak and an incomplete removal of wrapped feedback-index descriptions; both were corrected and
independently rechecked. Release advice now says the tag is spent and directs the operator to fix main and cut the next
version. The regenerated workflow differs only in that advice.

There is no shipped product behavior change. All six feedback items are removed, and their ledger entries identify the
same completed change and draft PR.

## Open questions and possible follow-ups

No decision blocks this work. No outcome tripped its complexity gate or was dropped. A native macOS smoke check could
strengthen confidence in the portable process-start-time command; this run verifies the matching and refusal behavior on
Linux, not execution of macOS's `ps`.

## PRs

- [#1799](https://github.com/scode/farhelm/pull/1799/changes): protect unrelated work and private information in
  maintainer tooling; one commit, based on main, left as a draft.

## Checks run, reused and skipped

- `cargo fmt --all -- --check` and `cargo clippy -p farhelm-supervisor --all-targets -- -D warnings` passed. The
  isolated `python -B scripts/check-test-sleeps.py` inspected 281 calls with zero unannotated delays. Changed-Markdown
  `dprint check` passed.
- The two changed probe tests ran with a temporary path containing spaces through the recorder, pinned nextest, four
  slots and zero retries, without tmux. Run `a55ab2c9-bc93-4475-bc70-f12757a306b8` passed the escaping-descendant case
  but failed the inherited-pipe descendant-liveness assertion. Orphaned sleep processes were observed as zombies under a
  container PID 1 that did not reap. The exact failing case passed on unchanged source with init reaping enabled in run
  `47bbd0c5-2ca7-4402-be71-1a86c01a955c`. Both observations are retained; the comparison supports a substrate cause, not
  a new source fix.
- Controlled direct proofs passed for matched and stale process identities, status/stop/wait behavior, preservation of
  an unrelated live process, and full/short hostname scrubbing including uppercase qualified names: run
  `ebde8ab3-0149-4823-a094-6f84e9c2e039`. The original direct pass `bf67716c-bf02-4866-a435-0ad81c998c67` lacked the
  uppercase case. Run `3d1d32a6-d55e-460e-a892-bcc56bedc379` failed during scratch-fixture setup before testing product
  behavior; it is retained separately.
- The source reviewer reproduced the original uppercase hostname leak in run `1835452c-b783-451d-8882-74131aa27f6c` and
  verified its correction in run `bbdcf9a5-40ea-4747-a7ad-41c61d235c1f`.
- `npx --no-install playwright test spawn.spec.ts --list` passed syntax discovery for all six Chromium/WebKit cases in
  recorder run `9de558ef-3d1b-4f33-b467-95700c26e76c`. The opt-in real-agent scenario and screen capture were skipped
  because they spend vendor turns; their changed path/name consumers were reviewed directly.
- Pinned cargo-dist 0.32.0 regenerated the release workflow and `dist generate --check` passed. A release was not cut.
- The required isolated `deflake/EVAL.md` procedure passed on Linux in sweep `9b5ddd8a-122d-44c1-93d2-608aeb32e84a`,
  with a fresh low-power agent requested on gpt-6-luna high. It received the expected one-shot Rust flake (three passing
  classification reruns), the planted deterministic JavaScript assertion failure (three failing reruns), then
  sweep-finished. It recorded the flake and fixed the assertion in separate throwaway commits. Five wait calls ended
  with one limit exit 3, three event exits 0, and final exit 4. The executor inspected the complete report, actual event
  records, commit contents, finished state, and absence of sweep build output; owned evaluation resources were then
  removed. No tooling ambiguity or daemon/phase/cleanup failure was reported. This is tooling evaluation, not a new
  real-flake finding. A second native macOS evaluation was skipped because the change uses the plan's existing portable
  `ps -o lstart= -p PID` interface and compares its output without platform-specific parsing; the Linux evaluation
  covers both writers and the shared matching/refusal workflow. Source review checked that portable shape. Actual macOS
  command execution remains unverified, as noted above.
- Validation covers source revision `38128baa`. It remains applicable after rebases onto `467f2829` and `59e8b641`,
  containing only queue/report deliveries, appended triage decisions and unrelated feedback removals, and the ledger URL
  amendment. The full upstream diffs were read. A ledger conflict was resolved by retaining all new triage entries and
  all six completion URLs; the resulting diff was checked and Markdown formatting passed. No tested behavior or
  governing contract changed, so runtime checks were reused. Broader workspace, browser, desktop, installer and release
  execution checks were skipped because the changed runtime contracts are confined to the focused maintainer tooling
  above. No hosted CI was requested.

## Review gate outcome

The required fresh source reviewer was requested on native gpt-6.1-sol at high effort and reviewed correctness, design,
idiomatic code, the ledger criteria and the full test-authoring checklist. Its final artifact clears both findings and
reports no open code findings. Two fresh gpt-6.1-sol medium wording readers were used; the accepted second draft conveys
the destructive triggers and privacy/release motivation without a contradicted claim or convention violation. Native
actual model identity and usage counters were unavailable; a late recording gap for the second wording launch and review
continuation is preserved privately. Implementation was performed locally under no-workhorse mode. The executor did not
mark the PR ready or merge it.

### Landing

Landed on 2026-10-10 (UTC) as #1799. This round landed five triage plans together, in order: untrusted-text-escaping,
git-env-isolation, os-readback-fixes, harness-tooling-fixes and ssh-config-atomic. Each rebased onto main with only
conflicts in the review queue's index, where each plan removes only its own entries. Since their stacks were based, main
gained this day's earlier landings (sounds, file downloads, the reboot follow-up of the supervisor's timer sweep) and
the 2026-10-10 spec triage; of the files these plans touch, only the helm's supervisor client changed upstream (download
routing), away from the log line one of them changes. A separate reviewer read all five against each other and main by
reading the code only, and checked each against its triage decisions and completion criteria.

#### A fix made while landing

The deflake tool now records its daemon's start time and later compares it to decide whether a recorded process is still
its own. It read that time through `ps` in the caller's time zone and locale, so a later `wait`, `status` or `stop` from
a shell with a different time zone would see a mismatch: a live run would be reported dead, or `stop` would leave the
daemon running and holding its lock. The landing pinned `ps` to UTC and the C locale. In #1799 before it merged.

Left as it is: two comments in the release build's setup file still describe deleting a failed tag, contradicting the
corrected advice; they do not reach the generated workflow.

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

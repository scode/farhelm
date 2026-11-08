## Since the last review

The earlier execution blocked on the size allowance before changing code or opening a PR. The answered plan now has its
first implementation and draft PR, #3173; no earlier PR was changed or dropped.

## What this was about

Delivering recipe credentials to a Tensorlake host could silently undo a Claude Code or Codex settings edit saved while
Farhelm prepared the delivery. Triage chose to refuse replacing a file that changed after it was read, merge once with
the new contents, and leave the existing amber warning if another save conflicts again.

The final replacement now checks the old contents or expected absence. A second conflict preserves the saved file and
leaves delivery unrecorded, so the next connection tries again.

## Things you should know

The landing monitor raised the original approximate 100-line allowance to about 280 after the first execution found the
shared installer needed an old-content condition. The measured code, tests and specification change is 300 added plus
removed lines, near that approximate allowance. It reuses the uploader and existing warnings and delivery records; it
adds no lock, host protocol or retry task. Other installer callers retain unconditional installation.

The comparison and replacement run in the same finishing command on the host. A save made after the final comparison but
before replacement can still be overwritten; the check detects edits already present when that comparison runs.

## Open questions and possible follow-ups

An agent can still save its own stale settings after Farhelm successfully delivers credentials, undoing the new token.
That reverse direction was explicitly outside this outcome and remains unchanged.

## The PRs

- [#3173](https://github.com/scode/farhelm/pull/3173/changes): preserve concurrent agent settings edits; draft, based on
  main.

## Checks run, reused and skipped

- Passed: recorder-backed `cargo nextest run -p farhelm-helm --lib` selecting the credential-delivery module and
  `conditional_settings_install_preserves_a_concurrent_save`. Run `90be4abe-4080-4fbe-954a-387afa5f4bae`: 24 passed,
  1,544 selected out.
- Passed: the same command selecting existing local atomic convergence, remote setup ownership, upload failure cleanup,
  orphan cleanup and upload tampering tests. Run `d3e74cd4-ec14-45e5-9524-7db278f58ec3`: five passed, 1,563 selected
  out. Both runs used pinned nextest 0.9.148, four slots, zero retries and `--tmux none`; they use owned files and local
  shell stand-ins, not terminals or cloud services. Source capture, runner reports and retained output are complete,
  with no runtime `SKIPPED` marker or cleanup limitation.
- Passed: `cargo fmt --all -- --check`, `cargo clippy -p farhelm-helm --all-targets -- -D warnings`, the isolated
  `python -B scripts/check-test-sleeps.py` check (423 delays, zero missing rationales), targeted `dprint check`,
  `python3 releasing/check-changelog.py format`, and `python3 scripts/check-agents-md.py`. The last reports zero errors
  and three existing unrelated citation warnings.
- Reused for the final draft: these checks covered base `fcb7349b` plus the implementation working diff. The subsequent
  rebase onto `f164d321` has an empty interdiff; upstream changes were plan claims and a separate plan's report. Later
  edits only add the PR URL to the ledger. No executable, test or specification content changed.
- Skipped: other Rust suites, browser, desktop, installer and cloud runs. The selected tests cover the changed delivery
  contract and the shared installer's unchanged ownership, convergence and cleanup behavior; no broader coverage gap was
  identified. The installed Farhelm was not changed.

## Review gate's outcome

The required fresh-context gpt-6.1-sol reviewer at high effort found no actionable correctness, design or idiom issues.
The independent resume check and commit-message cold read passed. No finding remains unresolved.

## What this was about

Starting a session in a fresh GitHub checkout failed on hosts using Git's `safe.bareRepository=explicit` setting.
Farhelm created its bare repository cache, then Git refused the fetch because the cache was selected through implicit
repository discovery. The checkout, hook and agent never ran.

The agreed fix names Farhelm's own cache explicitly during fetch. The host's restrictive policy stays enabled. The
implementation specification now shows that command and its reason, and the original feedback item is removed.

## Things you should know

The fix stayed within the agreed scope: the cache fetch's repository selector, its tests and the matching specification.
The cache and clone design did not need a change. The real-Git regression starts with an absent cache, first proves that
implicit discovery is refused by the policy, then reaches a ready checkout and runs the agent under that same policy. It
also checks the checkout's tracked content and cache object directory. Git configuration is supplied only to child
commands; the test process's environment is not changed.

## Open questions and possible follow-ups

None required for this outcome. The regression uses an offline local upstream reached through Git's URL rewriting; it
does not exercise GitHub networking or authentication, and it does not audit other Git safety settings.

## The PRs

- [#2199](https://github.com/scode/farhelm/pull/2199/changes): honor explicit bare-repository policy in checkouts. Draft
  head `9c1fb40fdccc720fec021ecb8485bba3cdfacf5c`, bookmark `plan/bare-cache-explicit-git-dir/01-git-dir`.

## Checks run, reused and skipped

The runtime run captured base `e29365df` plus its complete working patch, fingerprint `ce76b2a0` and tracked-diff hash
`a300b8b4`. Later edits before review only completed the triage ledger and feedback cleanup; executable source stayed
unchanged. The reviewed source revision was `799b3a86`.

- Run: recorded
  `cargo nextest run -p farhelm-supervisor --lib -E 'test(d1_fake_git_hook_agent_run_in_order_with_exact_argv) | test(d2_real_git)'`,
  recorder `952efe6a-bf2a-4f14-b7fd-9aa0ac14c961`: all five selected tests passed, including the new explicit-policy
  regression; 1,130 tests were selected out. Pinned nextest 0.9.148, four slots, zero retries, two cargo build jobs,
  Linux and Git 2.43.0. These shim tests do not use tmux. The report and retained output are complete, with no runtime
  `SKIPPED` diagnostic, output loss or forced cleanup.
- Run: `cargo fmt --all -- --check`, changed-file `dprint check`, `python3 releasing/check-changelog.py format`,
  `python3 scripts/check-agents-md.py`, and `git diff --check`: passed. The isolated test-sleep checker inspected 302
  delays with zero missing rationales.
- Run: `cargo clippy --all-targets -- -D warnings` passed with two build jobs, on the reviewed source before rebase.
- Reused: the successful runtime, source and static checks above after a clean rebase onto `07d244e6`; the patch was
  byte-identical across that rebase. Main added the browser input-method key guard and helm download-admission cleanup,
  plus their specification and bookkeeping. Their complete diffs were inspected: neither changes supervisor launch, Git
  environment handling or the repository-cache contract. The final draft differs from the rebased checked patch only in
  its triage-ledger PR link; Markdown formatting and whitespace checks passed again for that update. Subsequent main
  changes inspected before delivery were plan states and landing notes only. No runtime rerun was warranted.
- Skipped: the full Rust suite, browser, desktop, installer and provisioning tests. The changed behavior is the
  supervisor's Git cache fetch, covered directly by the exact-argv test and all four existing/new real-Git preparation
  tests. No UI, installer or host-management behavior changed.

## Review gate's outcome

The required fresh-context gpt-6.1-sol high source reviewer found no correctness, design, idiom or complexity-gate
violations. It checked the full diff, the real-shim test path, policy isolation, spec and feedback bookkeeping. Its
review was source-only; the recorder run above supplies execution evidence. A fresh gpt-6.1-sol medium reader understood
the commit and PR wording and found no contradicted claim or convention violation. The first wording task arrived as an
unreadable payload; it was resent as plaintext before that reader saw the candidate. No implementation was delegated.

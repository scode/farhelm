## What this was about

Starting a session in a managed GitHub checkout could make the user wait while Git consolidated Farhelm's repository
cache, with cleanup output in the session terminal. The maintainer chose to move that work to the existing background
worker, including when prepared checkouts are disabled.

Launch-time cache fetches, clones and prepared-checkout updates now disable automatic Git cleanup. After cloning ends,
the background worker either builds a needed spare, consolidating during its cache refresh, or consolidates the cache
alone. These modes share the existing login-shell helper and never run cleanup twice for one coalesced attempt.

## Things you should know

Cleanup can begin while a configured post-clone command is still running. It waits for cloning to end with prepared
checkouts enabled or disabled. A later launch can still wait if background cleanup already holds that repository's cache
lock; the maintainer explicitly accepted this tradeoff. No additional waiting-launch optimization was added.

The existing pack threshold, cache location and eviction age, prepared-checkout coalescing, cancellation and deadline
rules remain. Cleanup-only work does not extend the cache's last-checkout age. User repositories and their later Git
commands are unchanged. The live installation was not used.

## Open questions and possible follow-ups

No required work remains. Avoiding the accepted lock wait would require a separate scheduling change.

## PRs

- [#3130: defer checkout cache cleanup to the background](https://github.com/scode/farhelm/pull/3130/changes) — draft,
  commit `8edef7d72ae19297ccf8abec05e91ab3fc38ba89`.

## Checks run, reused and skipped

Runtime checks use the repository recorder, pinned nextest 0.9.148, four slots, zero retries and required pinned tmux
3.7c. Excluded tests are outside the selected scope.

- The supervisor selection passed 100 tests in `1557d00d-8db3-4376-9438-762e0306cc28`:
  `cargo nextest run -p farhelm-supervisor --lib -E 'test(/^launch::/) | test(/^spare_take::/) | test(/^spare_build::/) | test(/^service::spare_builder::/) | test(spare_builder_)'`.
  It includes the real-Git accumulated-pack proof, launch and spare-update overrides, cache-lock and eviction-age proof,
  disabled-spare clone gate, and existing coalescing/publication/cancellation checks. The recorded working tree was
  based on `bf9664c5`, source fingerprint beginning `7c6d3895dc256370`; the pushed revision has identical Rust behavior.
  Later edits only clarified specification prose.
- The shipped helper and managed-checkout selection passed 18 tests plus fixture setup in
  `ca563242-4069-4028-a913-e42dd5965fd1`:
  `cargo nextest run -p farhelm --test spare_builder --test e2e -E 'binary(spare_builder) | test(/^github_checkouts::/)'`.
  It covers real guarded cleanup-only success/failure, login-shell receipt refusal, owner-death and deadline cleanup,
  ordinary GitHub checkout creation/replay, and taking and replenishing prepared checkouts with the frozen switch. Both
  recorded runs have complete JUnit reports and retained output, matched the pinned substrate, and contain no runtime
  `SKIPPED` notices. Their Rust source matches the pushed revision.
- `cargo clippy -p farhelm-supervisor --all-targets -- -D warnings` and `cargo clippy -p farhelm --bins -- -D warnings`
  passed, covering tests and the shipped CLI configuration.
- `cargo fmt --all -- --check`, dprint on changed Markdown, changelog format and diff whitespace checks passed. The
  pinned source-only test-delay checker inspected 420 delays with none missing a rationale.
- The initial compile failed with a missing Rust error-type annotation, before any runtime test ran, in
  `8832374c-1080-4631-b3fa-fb8b964f2251`. The annotation was corrected before the passing supervisor run. Failure
  evidence is retained; this was a same-session compile correction, not an established latent flake.
- No earlier plan's runtime evidence was reused. The full Rust battery, browser and UI tests, native desktop,
  installer/release suites and GitHub CI were skipped because this change is confined to checkout preparation and its
  existing background helper. The selected supervisor and shipped-process checks cover those paths. No UI code changed.
  No instruction, heading or path was renamed, so the agent-instructions checker was not needed.

## Review gate

One fresh-context gpt-6.1-sol reviewer at high effort reviewed the complete change against the plan and the repository's
test-authoring checklist, supplied verbatim. It found no actionable issues. A separate documentation pass covered every
touched file. The final PR text passed a valid fresh-context cold read after its first title's timing claim was
corrected.

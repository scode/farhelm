## What this was about

An agent can stop at a trust prompt even when Farhelm is configured to pre-trust the workspace. Older drift checks could
answer the question and still pass. The maintainer asked for checks that leave the dialogs unanswered, cover both
explicit choices on Codex and Pi, and put Claude's config in a private copy for Farhelm's trust edit.

The new `workspace_trust` category launches fresh directories in the installed CLIs and waits for the composer without
approving anything. Codex's explicit untrusted launch must show restricted folder access. Pi must load an inert project
extension when approved and explicitly say that it ignored project resources when approval is disabled.

## Things you should know

Claude and Codex passed on the final revision. The earlier five-agent run supplied passing evidence for Antigravity,
Muse and Pi. Antigravity and Muse opened their composers without a trust dialog. Codex accepted the expected
workspace-specific argument in both cases, reached its composer when trusted and offered “Open restricted” for the
explicit untrusted launch. Pi loaded the extension only when approved and displayed its ignored-resources warning for
the other launch. The focused rerun covered Claude and Codex after the patch moved Pi's approval-question check earlier.

Claude's focused run reached the composer, found the new workspace's trust entry in the private config and verified the
real config's bytes were unchanged. The fixture preserves the vendor login, copies its trust files into the test
directory and directs the launch there before Farhelm resolves and edits the file.

## Open questions and possible follow-ups

The first full run found the expected private Claude trust entry but failed its byte comparison on the real config
(`2804bb90-4132-4303-9e31-77d9baaa3f00`). A separate check did not find the failed test's workspace entry in the real
config. The cause of the change is unknown. The focused rerun passed the same strict comparison, and the original
failure remains retained.

## PRs

- [#4794](https://github.com/scode/farhelm/pull/4794/changes) adds the workspace-trust category and the isolated Claude
  trust fixture.

## Checks run, reused and skipped

- Through the recorder, `cargo nextest run --locked -p farhelm --test e2e -E 'test(/^real_agent_capture::drift::/)'`
  passed 24 tests at `ec497613` (`744f3eb0-d67c-46c6-9289-54b987d2c401`).
- The recorder captured 18 passing Python runner checks at `36e4eea2` (`e993305a-72c3-43f7-a78a-6772247aba6e`) using
  `python3 -B scripts/test-run-harness-drift.py`. No subsequent change altered its runner or the five registrations.
  `python3 -B scripts/run-harness-drift.py --list --category workspace_trust` discovered all five at `4fb381ea`.
- The recorder captured two passing production launch-shim checks with disposable Claude configs at `e05bd3f9`
  (`f968f11c-b402-4bfd-b7cd-c2a32fa5c67b`), using
  `cargo nextest run --locked -p farhelm-supervisor --lib -E 'test(/plain_claude_launch_/)'`. The launch shim did not
  change.
- `python3 -B scripts/run-harness-drift.py --no-upgrade --category workspace_trust` ran all five on `4fb381ea`. The
  subsequent run of that command with `--harness claude --harness codex` passed on `ec497613`. Antigravity, Muse and Pi
  reuse the earlier evidence. Antigravity and Muse keep the same composer and question cues. Pi showed the same cues and
  passed the extension and refusal checks. The final patch checks its approval question earlier.

| CLI         | Installed version | Passing recorder run                   |
| ----------- | ----------------- | -------------------------------------- |
| Antigravity | 1.5.16            | `c3a6511c-485b-4c41-944e-e83e729d1ecb` |
| Claude      | 2.1.332           | `4b5bb20b-d6e4-41b9-88bf-3ffb96fc58df` |
| Codex       | 0.172.0           | `1a78a125-b36a-4d94-8ef6-fa7f580b8074` |
| Muse        | 1.4.25-R9361.1    | `8fd70e15-9db0-4983-90cc-4aa90572de53` |
| Pi          | 1.6.0             | `14b074cc-b9a1-43db-ab15-333574dc153c` |

- `cargo clippy --locked -p farhelm --all-targets -- -D warnings` and `cargo fmt --all -- --check` passed on `ec497613`,
  the draft's tree. Markdown, test-delay and instruction link checks passed on `4fb381ea`. The later edits only move
  startup checks and reuse the shared polling pace. Instruction checks reported one pre-existing archive heading
  warning.
- Vendor upgrades, model turns, the full workspace suite and desktop tests were skipped. The trust behavior is
  observable at startup, and the plan changes only opt-in drift checks and their TODO entry.

## Review gate

The independent Sol review caught paths that could have redirected Claude's trust edit or copied a protected file back
through an alias. These were repaired and verified with disposable files before launching Claude. It also identified
Codex's early composer cue and Pi's missing refusal evidence. The final review examined the updated behavior and found
no remaining code findings. The scope review and draft-title cold read passed.

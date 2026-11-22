# Agent trust in every folder

## What this was about

Claude Code could still ask whether to trust a folder you chose after you enabled **Answer Claude's "trust this
folder?"** in Farhelm Settings. Claude, Codex, Muse and Pi previously limited their switches to managed checkouts, the
repository copies Farhelm clones and manages. Their switches now apply to a newly composed launch in any directory,
including agent requests, Clone, Restart with and Replace with. Plain Restart and Replace reuse the saved launch. The
corresponding hints can appear in ordinary folders while the question is visible and the switch is off.

## Things you should know

- A switch you already turned on now applies in directories you choose. Farhelm attempts to record trust for the folder
  in Claude's configuration on the host before launch. After a successful edit, the folder stays trusted after the
  session ends. That also applies to a managed checkout cloned with the switch off and later opened with it on. For a
  checkout cloned with the switch on, Farhelm still tries to remove its entry when you archive the checkout, even if it
  first started another agent. A wrapper that changes Claude's directory or configuration can still leave Claude asking.
- In a startup-only local probe, Claude Code 2.1.330 showed “Yes, I trust this folder” in a plain folder, Git root and
  nested Git folder without a trust key. Adding the exact directory's key reached the composer in all three. A root key
  also worked for the nested folder. Codex 0.171.0 showed “Trust this folder?” at a Git root and in a Git subfolder. Its
  existing per-launch trust option keyed at the launch directory reached “Ask Codex to do anything”. The tested plain
  non-Git folder reached the composer even without the override. The captured Git root menu matched the existing
  recognizer. These observations used isolated configuration and synthetic credentials. No model prompt was submitted.
  No separate verification PR was needed.
- The Claude change bumps the supervisor protocol to 84. Release testing should cover an updated helm with an older
  supervisor. If that supervisor accepted the new launch, it could start Claude without the config edit and leave the
  user facing the folder-trust prompt.

## Open questions and possible follow-ups

- When an agent requests a trusted launch, Farhelm resolves the chosen folder for the approval card and dispatch.
  Retargeting the original symlink cannot change that path. Replacing the resolved directory or one of its ancestors can
  still redirect the launch and leave that other folder trusted in Claude's config. This is the most consequential
  follow-up. Binding the approval to a directory identity through process startup needs a separate change.
- A client retrying the same direct create through a symlink with the same idempotency key across the upgrade can get a
  conflict. Previously a trusted managed launch stored the resolved directory in the request fingerprint. The current
  path uses the caller's spelling and ordinary supervisor checks. This compatibility case is deferred.
- Muse can still ask about trust when a Custom argument list has a bare `--`, because its trust flag lands after the
  option terminator. Codex's integration flags have a related ordering limitation. Extending trust to ordinary folders
  makes this reachable in more launches. A separate fix would need to preserve existing saved commands.

## PRs

- Apply the Codex, Muse and Pi switches in any directory and update their Settings and hints.
  [#4567](https://github.com/scode/farhelm/pull/4567/changes)
- Extend the Claude switch and hint, retain managed-checkout cleanup, and bump the protocol.
  [#4572](https://github.com/scode/farhelm/pull/4572/changes)

## Checks run, reused and skipped

- The first PR passed 47 of 47 using `cargo nextest run -p farhelm-helm -p farhelm-gui --lib` with a filter for
  `workspace_trust|normalize.*trust|pin_a_plain_alias|composition_incarnation|agent_clone_keeps_the_trust_choice|session_pane::trust_band|settings::tests::.*trust`,
  wrapped by the recorder (`f2decca6-948f-49cf-bac3-58f203da07a6`). Subsequent copy changes passed the website build
  (`bun run build` under `website/`) and strict GUI Clippy. The full golden update added eight intended renders
  (`4d9660fb-d15b-40c5-80f8-c8427ca3cb1e`).
- The full stack passed 52 of 52 using
  `cargo nextest run -p farhelm-proto -p farhelm-supervisor -p farhelm-helm -p farhelm-gui --lib` with a filter for
  `workspace_trust|stored_trust|normalize.*trust|plain_claude_launch|claude_trust_launch|session_pane::trust_band|version_skew_names|protocol_version_is_pinned|pin_a_plain_alias|composition_incarnation`
  (recorder `8c3e8e7b-b5d0-4251-a395-b34ae7e481c1`). The two successful nextest runs logged no runtime skips.
  `cargo clippy --locked --all-targets` with `-D warnings` passed for each PR's changed packages.
  `cargo fmt --all -- --check`, `python -B scripts/check-test-sleeps.py` and
  `python3 releasing/check-changelog.py format` passed. An earlier build stopped on a type mismatch before reaching any
  tests (`ac6af283-b9bd-4fc9-9ddb-202ec80805e2`). The successful run includes the fix.
- Reused the 52 tests and four-package lint at `5403cc905214` after comment and wording changes. The final website build
  passed. The full golden update changed Claude's Settings page and added its hint in light and dark (recorder
  `725fbc87-4287-4068-ab60-57dc6548f944`). Both updates used `python3 -B scripts/check-gui-goldens.py --update`. The
  instructions link check reported four existing warnings and zero errors.
- Rebased both drafts onto `193043abdff7`. Main's shell-template change preserves the workspace-trust choice. Main's
  launcher and shortcut fixes act on different inputs and leave launch trust and the protocol unchanged. The rebase
  preserved the two patches and both sets of screenshot references. `python3 -B scripts/check-gui-goldens.py` compared
  the six trust Settings and hint scenes and the new shortened session-list scene, using seven `--scene` selectors. All
  14 pictures matched (`d5dafa1d-9507-40b6-8467-213918ae2393`). Later container and host changes use separate screens
  and do not change the compared scenes.
- Skipped the full Rust suite, release packaging and macOS checks. The focused launch and protocol cases cover the
  behavior changed here, and the visual gate used Linux headless renders. The installed-agent probe spent no vendor
  turns. CI runs on demand.

## Review gate

Both PRs cleared the GPT-6.2 Sol high source gate after corrections to the tests, help and specs. The remaining cases
are recorded above as follow-ups. Opus 5.5 high checked every changed scene in light and dark and found no design
mismatch. Both commit messages passed a cold read.

### Landing

#### What landed with it

The plan's two PRs (#4567, #4572) landed second in this round, after the launch that resumes a suspended host, with a
landing fix, #4583, followed by SQLite maintenance and the test worker's staging lock. Since the plan's work was
written, main had gained the desktop app triage and the Tensorlake triage, which also changed the helm's agent requests
and sessions; there was no conflict. The supervisor protocol goes from 83 to 84, the only bump in the round, and it
covers the change.

An independent reviewer read the stack against the plan's acceptance criteria and Decisions. It found no blocker, but
one serious consequence, fixed below.

#### Fixes made while landing

All in [#4583](https://github.com/scode/farhelm/pull/4583/changes):

- **Claude trust in the home directory.** With Claude's switch on, a launch now marks its directory trusted in Claude's
  settings file in any directory, including the home directory, which the launcher falls back to and which agent
  launches without a repository use. Claude treats a trusted directory as covering every directory inside it (the
  executor's own check showed this), so one launch in the home directory would make Claude trust everything under it:
  outside Farhelm too, and after the switch is turned off, since these edits keep no removal record. That goes well
  beyond the accumulating entries you accepted. The landing narrowed your "every folder" decision only there: the
  launch-time edit now skips the home directory and the filesystem root, prints a short note in the session's terminal,
  and Claude asks its own question there. SPEC.md, the Claude docs page and the changelog fragment say so. This judgment
  call is recorded in TODO.md's "Plan decisions to revisit"; the alternative was to apply it everywhere with a warning.
- **Claude creates and reconnects.** Every agent request to start a Claude session now went through the approval step
  that remembers which host connection checked the folder, even with the switch off, so a helm reconnect while the
  approval card was open refused the request. The connection is now recorded only when the folder is checked.
- **A broken test and a missing list entry.** The settings search test looked for a phrase the plan removed from
  Claude's caution, which failed the workspace suite; five new trust screenshots were missing from the list of
  screenshot scenes, which that list's own check fails on. Three SPEC.md sentences and several code comments still
  described trust as limited to managed checkouts; they now describe any folder.

#### Checks

This round landed four plans stacked in this order on main: the launch that resumes a suspended host, agent trust in
every folder (protocol 84), SQLite maintenance, and the test worker's staging lock. The checks ran on the combined
result. The worker runs used this round's own test worker harness, so they exercised it too; they ran on a separate
arm64 Linux test worker, and the rest ran locally on x86_64.

On the four plans as delivered, stacked onto main:

- `cargo nextest run --workspace --exclude farhelm-desktop`, with four nextest slots, no retries and four build jobs, on
  the test worker (recorder run `fa0b5b85-0c85-4685-a517-30a4dc406bf9`): 8304 of 8308 passed. Two failures were the
  worker running out of disk during the run (helm provisioning over SSH and the real Podman environment test, both "no
  space left on device"); the other two were the GUI tests the trust and wake plans' landing fixes correct.
- The four clippy runs (`--all-targets`, `-p farhelm --bins`, and the desktop app with and without `--features agent`),
  on the test worker, not through the recorder: two errors, which the wake plan's landing fix corrects.
- `cargo nextest run -p farhelm-desktop`, on the test worker (recorder run `99f4ecdb-f251-4c4c-bdcb-84747ee38556`): all
  184 passed.
- `python3 -B scripts/gui-e2e-smoke.py`, on the test worker (recorder run `0b374828-a0ef-49ad-9af6-966eeace3dad`): 49 of
  its 50 steps passed and the paste step was skipped, as on every Linux run.
- `python3 scripts/check-gui-goldens.py`, every scene, locally (recorder run `dbe2bda8-bda8-4af3-b9b6-ffc64f569658`):
  passed.
- `python3 -B scripts/test-worker/test-test-worker.py`, the worker harness's own tests against a local SSH stand-in,
  locally (recorder run `f6f57fc8-5f99-4ac8-8038-9994d9ce98de`): all 9 tests passed (the report counts eight scenarios);
  `shellcheck -x` on the harness scripts and `python3 -m py_compile` on its Python files: passed.
- The docs website build (`bun install --frozen-lockfile && bun run build`), `cargo fmt --all -- --check`,
  `python -B scripts/check-test-sleeps.py` (in its isolated environment), `dprint check`,
  `python3 releasing/check-changelog.py format` and `python3 scripts/check-agents-md.py`, locally: passed.

After the landing fixes (#4582, #4583, #4584):

- `cargo nextest run --workspace --exclude farhelm-desktop` again, on the test worker with free disk (recorder run
  `442411a7-47a3-448c-be2c-fb6e4582df74`): all 8309 passed, among them the two tests the full disk had failed, the two
  corrected GUI tests and the new home-directory test.
- The four clippy runs, on the test worker: passed.
- `python3 -B scripts/test-gui-screenshot-batch.py` (which would have failed before #4583 added five trust screenshots
  missing from the screenshot list), the docs website build, `cargo fmt --all -- --check`,
  `python -B scripts/check-test-sleeps.py`, `dprint check`, `python3 releasing/check-changelog.py format` and
  `python3 scripts/check-agents-md.py`, locally: passed.

The first runs' desktop, smoke, golden and harness results still apply. The fixes change no desktop-app or harness code
and nothing that draws: in the GUI library a removed `&`, a type alias, comments and tests; in the helm and the
supervisor only Claude agent requests and Claude trust launches, which the smoke test's stand-in agents never make.

Skipped: a launch onto a real suspended Tensorlake sandbox or stopped container, and a real Claude launch with trust on,
which spend cloud resources or vendor turns; macOS, since no Mac was part of this landing; CI, which runs only on
demand.

#### Things you should know that the report does not say

- The Settings caution box does not mention the home-directory exception; changing its text would have meant
  re-rendering that screen's reference pictures. SPEC.md, the docs page and the release note do.
- Agent environment launches without a repository start in the sandbox's home directory, so with the switch on Claude
  now asks there instead of being answered for.
- Codex, Muse and Pi are not exposed the same way: their trust is passed per launch and stores nothing.
- Claude's trust hint now appears over Command-tab Claude sessions in any folder, offering a switch that never applies
  to Command launches.
- No test checks that a session's first launch and its Restart both carry the new Claude trust setting from the saved
  launch.

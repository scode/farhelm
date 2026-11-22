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

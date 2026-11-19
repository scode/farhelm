# SSH sign-in prompts

## What this was about

The six draft PRs are ready for the landing monitor. This report describes the behavior in that stack.

A remote host that asked for a verification code or password could stay disconnected until you ran SSH by hand. That
could happen at startup, when adding a host, or after a Wi-Fi change while you were working in another session.

Farhelm now shows the questions OpenSSH passes to askpass when connecting to a host from your SSH configuration. **Add
host** shows the question inline while it checks the destination. Manual setup, Update, Uninstall and connecting for
sessions can also ask. For a registered host, choose **Answer** above the session list. The navigation rail shows an
amber key.

The maintainer chose an in-app question with a focus rule. It can open with its answer field focused before the first
key or click at startup, or while you are waiting for a host action you just started. Sending from the focused answer
field can make the next question open while you are still waiting. If you have started working elsewhere, the question
waits in the sidebar and your terminal keeps focus.

When the network goes silent, both shared SSH connections now detect the loss in about a minute and can try to
reconnect.

Farhelm uses a fresh reply id for each displayed question and passes its answer to SSH in memory. If sign-in for
sessions fails after showing a question, including cancellation or a two-minute expiry, Farhelm ends that sign-in
process and the host shows **Sign-in needed** until you choose **Sign in**. Failure before the first question stays on
the ordinary retry schedule.

## Things you should know

- Add host may ask once for setup and once for sessions. If the setup connection goes idle while the setup confirmation
  is open, it can ask again when you confirm. Cancelling the inline question keeps the destination and alias.
- Manual Uninstall rechecks whether the supervisor has running sessions after the SSH wait, since someone could start
  work while it was asking.
- The maintainer chose a brief amber cue for actionable rows, including sign-in, setup, update and host identity
  problems. An amber edge stays while the row needs action. Reduced motion uses a flash in place. The cue can still
  paint at the row's previous location for one frame when the row vanishes.
- **SSH sign-in** under **Settings → Notifications** controls the chime. It defaults to on.
- The security-key presence notice depends on the client exposing it to askpass. Tensorlake uses Farhelm's key. A
  compatible live SSH connection started by hand is still reused.
- If the helm's state-directory path is too long for an SSH control socket or contains an expression SSH would expand,
  key authentication can still work without connection sharing. If this attempt fails to connect, the error can name the
  path constraint that prevented Farhelm from showing a question. Setup needs one extra byte, so it can hit that limit
  while sessions still connect.
- During execution, automatic update discovery kept its batch checks so waiting for an answer on one host cannot hold up
  checks for others. If authentication keeps the discovery check from working, manual **Update** can prompt. An
  automatic update that has already started can ask if its setup connection needs signing in again.
- **SSH needs 2FA?** and its manual commands were removed. The website pages **Add a remote host** and **Send feedback**
  describe the new flow.

## Open questions and possible follow-ups

Check native macOS rendering and a live 2FA or security-key exchange through the desktop during release verification.
The Linux desktop tests and pictures exercise the field, focus, a fixture Duo-style menu and a fixture touch notice. The
live OpenSSH test exercises the CLI helper, including an actual key-passphrase prompt and handoff to a shared
connection.

## Pull requests

- Detect lost SSH connections with keepalives. [#4296](https://github.com/scode/farhelm/pull/4296/changes)
- Relay SSH questions for sessions. [#4299](https://github.com/scode/farhelm/pull/4299/changes)
- Desktop prompt, focus, status and chime. [#4300](https://github.com/scode/farhelm/pull/4300/changes)
- Sidebar attention cue and reduced-motion flash. [#4302](https://github.com/scode/farhelm/pull/4302/changes)
- Setup sign-in for Add host, setup, Update and Uninstall. [#4303](https://github.com/scode/farhelm/pull/4303/changes)
- Remove manual SSH help and update the docs. [#4305](https://github.com/scode/farhelm/pull/4305/changes)

## Checks

- Private OpenSSH 9.6 TCP relay, `python3 -B keepalive-probe.py` (033bec48-c5d5-4a23-8311-20dc8d9e592f). Both shared
  connections closed after 59.7 seconds when the relay stopped delivering traffic. The CLI askpass test,
  `python3 -B askpass-openssh-probe.py farhelm` (18954bc6-6891-45e8-bdec-9bebe688d191), relayed an unknown host key and
  a protected key's passphrase. An SSH control-socket check then passed. A real ProxyJump asked for the key passphrase
  once during a deliberately failed sign-in. Both probe commands are shown with path arguments reduced to file names.
- `cargo nextest run -p farhelm-helm --lib`, selected SSH arguments, process cancellation, answer routing, snapshot and
  migration tests, 28 passed (c015b084-681d-4d27-b54a-9dfc748e8ef8). `cargo nextest run -p farhelm-gui --lib`, sign-in
  and sound tests, 31 passed (4f8af6cb-aad8-4280-97e5-518b46d8ccef). Exact startup focus test passed
  (85f76bb2-8158-4990-847d-d16477b91482).
  `cargo nextest run -p farhelm-desktop --lib -E 'test(/desktop_filter_excludes_sensitive_websocket_diagnostics/)'`
  passed (ceb107dc-4fde-4300-bcc5-65ef2bf3f048).
- `cargo nextest run -p farhelm-gui --lib`, cue layer and sidebar attention tests, 25 passed
  (a4647ad3-3b08-4003-b0e8-0f82b443f339). In the first setup run (f9fc0836-c531-492e-a7c2-f0c0b715242e) the helm's SSH
  stand-in passed. The two desktop cases failed because the initial fake snapshot omitted the new collections. With the
  production collection shape they passed (d6a5cbf0-c829-45d6-96c7-8c7705a5b992). Later
  `cargo nextest run -p farhelm-gui -p farhelm-helm --lib` selections passed 56 setup, trust, sign-in and uninstall
  cases (d573bfe6-6473-4666-bcb6-f6b90fa96271). These checks cover answering during an Add host check, opening its
  confirmation even if the SSH connection is ready before the Send receipt, and opening the separate sessions question
  after setup finishes.
- The final `cargo nextest run -p farhelm-gui -p farhelm-helm --lib` selection passed 73 SSH configuration, probe, Add
  host, sign-in and Help cases (07df76bd-78f8-4c4e-8aa3-669bef8045f0). The first build was interrupted before any test
  ran (277d2d52-5939-44c9-8d6c-fa9240c12a3b).
- `python3 -B scripts/gui-e2e-smoke.py` passed 49 steps through a private Linux desktop, including Add host, Setup,
  Update, Uninstall and Help menu focus return (4144cfcb-3da4-48d9-8e64-92b11976563d). The large clipboard-paste step
  skipped because headless Mutter cannot give the app clipboard ownership. `python3 -B scripts/test-gui-e2e-smoke.py`
  passed all ten smoke-step discovery tests (a091d137-7fe5-4b30-9dad-46bb0920b0e0).
- `python3 scripts/check-gui-goldens.py --update` ran the full Linux capture suite at each visual step. It passed for
  the desktop prompt (a28eaaf7-c2bb-4754-9393-b1b046ff0d28), attention cue (6aaef940-bb8b-4520-a8cd-76f9c2fd433f),
  inline Add host prompt (79546317-3e26-4873-bfe8-5246dce5b20e) and final Help removal
  (d9659d3c-5942-400a-9713-f5b746f19c9a). Inspected every changed picture in light and dark before publication.
  `python3 -B scripts/test-gui-screenshot-batch.py SceneList.test_the_committed_scene_list_is_well_formed` passed for
  all four published manifests (1a8dac8a-bd91-47c2-8823-ff92cc6c02cb, cbdb5130-d6be-4cda-95f6-9b4352294594,
  65030250-4be6-461f-b89b-f4c2f001dd35 and aba73ff5-ca2a-4a35-a8f9-dd2c1f8bf94f). Development capture and catalog
  failures are recorded in the working log.
- `cargo fmt --all -- --check`, targeted `cargo clippy --all-targets -- -D warnings`, `dprint check`,
  `scripts/check-test-sleeps.py` and `releasing/check-changelog.py format` passed. `scripts/check-agents-md.py` found
  zero errors and four existing archival link warnings. The docs site built with `bun run build` and every internal link
  validated.
- The final runtime and lint runs used source e1e9befb. The final capture used 7ff7d0ba, which adds the inherited setup
  screenshot manifest. The published capture was the remaining change at the pre-rebase tip fb57ffe9. Reused those
  results at the final tip 8fcaf4e8 after `git range-diff` found equivalent patches for the first five steps and the
  sixth differing only in the TODO hunk. Main's test-worker changes were outside the local runs. `git diff --check` and
  `dprint check -- TODO.md` passed after the rebase.
- The full workspace and release gates were skipped. The local selections covered the changed transport, setup, focus
  and Help paths, and the private OpenSSH runs exercised the actual client and helper.

<details>
<summary>Exact nextest selections</summary>

The package commands above are abbreviated. Each ran through the recorder with four slots and zero retries. These were
the filters:

- Sessions, 28 cases:
  `test(/^ssh_sign_in::/) | test(/ssh_answers_reach/) | test(/snapshot_on_open_holds/) | test(/^ssh::tests::/) | test(/schema_90_adds|schema_85_adds|former_device_settings_are_sparse/)`.
- Desktop, 31 cases:
  `test(/^sign_in::tests::/) | test(/^sounds::/) | test(/each_sound_switch_saves_its_own_preference_field/)`.
- Startup recheck: `test(=sign_in::tests::startup_opens_the_question_and_send_can_focus_the_next_one)`.
- Cue, 25 cases: `test(/^layers::cue::tests::/) | test(/^session_list::attention::tests::/)`.
- Initial setup:
  `test(ssh_setup_preflight_returns_pending_then_allows_the_probe) | test(ssh_add_resumes_its_unchanged_probe_after_the_setup_master_answers) | test(cancel_inline_ssh_sign_in_keeps_the_destination_and_alias)`.
  The recheck kept the last two tests.
- Setup after review, 56 cases:
  `test(/ssh_add_resumes|cancel_inline_ssh|add_run_can_focus|^sign_in::tests::|setup_master|^provisioning::.*(uninstall|owned|duplicate|identity)|^ui_channel::.*setup_sign_in/)`.
- Final, 73 cases:
  `test(/^ssh::tests::|^help_menu::tests::|^hosts::add::tests::|^sign_in::tests::|^provisioning::.*probe|^tensorlake::hosts_tests::.*kind/)`.

</details>

## Review

The required independent source reviewer and cold message reader checked each draft. The attention cue uses the shared
layer manager and visible anchors recommended by the Opus 5.5 high design consult. The setup scope review recommended
reusing the existing UI operations and pushed collections. The source reviews caught early Ready versus Send receipt
ordering, session and ownership facts that could change during sign-in, and stale Help assertions and diagnostics. Those
findings were addressed before submission.

### What this was about

The startup Welcome card interrupted the first window and left the main pane with only a short placeholder. The agreed
design puts the introduction in that pane whenever Farhelm has confirmed there are no sessions. Help → Welcome opens the
same introduction on request.

### Things you should know

- The pane offers Start a session, Add a remote host, the Get started guide and Send feedback. Its marks follow the
  built-in agents in the launcher. At the minimum window width the links can wrap and stay centered.
- With an empty fleet, focus starts on Start a session, and Enter opens the launcher. When the first session arrives and
  replaces the focused pane, focus can move to the new terminal. In the Help dialog, a disabled Start a session gives
  initial focus to Add a remote host. Escape returns to Help, and closing a form opened from the dialog returns there
  too.
- Startup can show pending announcements followed by upgrade highlights. The stored legacy Welcome dismissal is carried
  for compatibility. The desktop reads and opens the introduction independently of it.
- The first-session guide now describes the empty pane. The legacy smoke step that opened a second app to check its
  startup card was removed. The existing smoke harness cannot seed a pending announcement at startup, so GUI and Python
  tests cover the override. The live smoke exercises Welcome through Help.

### Open questions and possible follow-ups

The brick-test operator walkthrough still describes dismissing the former startup card. Its automation already checks
conditionally for the card. A future brick-test documentation pass can remove the old instruction.

### Pull requests

- Welcome in the empty main pane. [#5012](https://github.com/scode/farhelm/pull/5012/changes)
- Welcome from Help and the end of the startup card. [#5018](https://github.com/scode/farhelm/pull/5018/changes)

### Checks

- `cargo nextest run --locked -p farhelm-gui --lib -E '<Welcome and startup selection>'` passed all 71 selected tests
  (run `73ca2fe4-efaa-4d6d-b296-9329599e5983`). The selection covered the empty pane, focus handoffs, Help actions,
  announcement ordering and upgrade highlights. `cargo clippy --locked -p farhelm-gui --all-targets -- -D warnings`
  passed.
- `python3 scripts/check-gui-goldens.py --update` passed all four Linux screenshot jobs on the rebased stack with
  coverage checks for both appearances (run `228d4f9d-8b29-4890-80e9-82916fca65dc`). The four empty-pane pictures
  matched their approved references. The six changes were the Welcome dialog and the two Help menu scenes in both
  appearances. What's New and the New container recipe matched main's updated references.
- `python3 -B` ran `scripts/test-gui-e2e-smoke.py`, `scripts/test-gui-agent.py` and `scripts/test-check-gui-goldens.py`
  (130 tests, run `7740a73d-8b5b-4a9e-ae0b-791d10c4d9d3`). The same interpreter ran
  `scripts/test-gui-screenshot-batch.py` (47 tests, run `e2bae7c8-0d2a-424b-a2c0-c19f6c53df10`).
- `cargo fmt -p farhelm-gui -p farhelm-proto` and `dprint fmt <changed Markdown files>` completed.
  `jj diff --git --from main@origin --to @ | git apply --check --reverse --whitespace=error` passed. Each changed shell
  script passed `bash -n <script>`.
- `python -B scripts/check-test-sleeps.py` and `python3 releasing/check-changelog.py format` passed.
  `python3 scripts/check-agents-md.py` reported the existing broken archive citation and no errors.
- Reused `python3 -B scripts/gui-e2e-smoke.py` on `7a1e3c0f` before the final rebase (run
  `61a1b040-bddd-4901-8085-5e04783fb38c`). It passed 48 steps, including Help opening Welcome and Escape returning
  focus. The platform-specific paste step was skipped. The rebase and review fixes preserved the automation IDs and the
  behavior the smoke exercises for Welcome. The new disabled-button case passed the focused GUI run after the rebase.
- Skipped the full Rust workspace suite and an installed Mac run. The scoped tests cover the affected GUI behavior and
  the Linux runs exercised the bridge and screenshots.

### Review gate

Separate Opus 5.5 high-effort reviewers checked source and screenshots for each PR. Source review prompted focus
coverage for the first session and the disabled Start a session action, accessible names for the agent marks, and
corrections to the Help wording and startup documentation. Both source gates cleared after those corrections. Final
screenshot review accepted the Welcome dialog and Help menu in both themes. The narrow pane wraps its links, and the
dialog uses the shared launcher-style header with the standard app glyph.

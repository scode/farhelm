## Since the last review

The landing monitor found three issues in the first delivery. It chose to keep the YOLO question available when a host
reports no installation identity and the answer can go back on the connection that refused the launch. A fresh launch
must ask again after a wake or reconnect. It also asked to report a successful Restart hook save when the launch has
changed, and to restore the exact repository suggestion alignment check.

#4999 now binds the one-time answer to the original connection when the host has no installation identity. It also pins
each edited Restart to the connection chosen at dispatch, so a reconnect the desktop has yet to see cannot redirect the
answer. A verified answer can still follow a recognized reconnection to the same installation. #5003 shows a separate
save acknowledgment and asks for a fresh press when the shown launch changes. #5037 verifies the repository suggestion
panel against the field and measures the scroll viewport separately for Recent flags. #5029 was restacked with the other
drafts. There are four draft PRs.

## What this was about

A suspended sandbox or stopped container could wake after **Start YOLO session anyway** and repeat the question. New,
Clone, Replace with and Restart with can now continue after wake or a checkout preview using the one-time answer for the
same verified installation. When an unverified host needs a wake or reconnect, the old answer stops at that boundary. On
the same live connection, the one-time answer sends the launch as usual.

When selecting Grok or Antigravity, **Set up and launch** enables the shared Restart hook setting for that agent. Future
launches use that setting on any host. A remembered folder could arrive while that save was pending, letting the launch
use a folder the confirmation had never shown. The answer now launches only if the draft was ready when the user
answered and still points to the shown destination. When the save succeeds after the launch changes, the form confirms
the save and asks for a fresh press. When the draft stays unavailable, it confirms the save and tells the user to press
the action once the draft is ready.

The launcher could report a saved Custom agent as deleted while its registration list was loading or a read had failed.
The Agent and Command tabs now keep the saved ID visible and explain the wait or failure. A retained entry from a
successful read remains usable, and only a successful list that omits the ID produces deletion advice. Restart with can
keep the required conversation field available while the registration is unread.

Keyboard users can now read the full Custom approval flags for the highlighted Recent setup before Enter. Both launcher
layouts keep those arguments below the scrolling results. Page Up and Page Down scroll long flags without moving focus
or choosing another setup.

## Things you should know

Edited Restart requests use the same connection check as Create and Replace with. Plain Restart continues to relaunch
the session's saved choice. After a visible and verified same-installation reconnect, the edited retry uses the new
connection and the original answer. A connection change that happens before the desktop can see it requires the user to
review the launch again.

The repository suggestion panel was already aligned with the input. The width test had followed the scrolling rows when
the panel's border moved into an outer element. The test now measures the painted panel, and the Recent flags test
measures the rows separately. Both keep their original geometric expectations.

## Open questions and possible follow-ups

None.

## PRs

- [#4999 Keep one-time YOLO approval through host wake](https://github.com/scode/farhelm/pull/4999/changes)
- [#5003 Tie hook setup to the destination shown](https://github.com/scode/farhelm/pull/5003/changes)
- [#5029 Distinguish missing Custom registration states](https://github.com/scode/farhelm/pull/5029/changes)
- [#5037 Show saved Custom approval flags in keyboard recents](https://github.com/scode/farhelm/pull/5037/changes)

## Checks run, reused and skipped

- The recorded `cargo nextest run --locked -p farhelm-gui -p farhelm-helm --lib -E <selection>` on `bf8f55c5` passed all
  189 tests (run `f133b639-343b-40a0-9f1b-73a2fa9b85b7`). It covered same-connection and verified-wake answers, stale
  restart routing, checkout alignment, narrow and wide Recent flags, hook setup, and folder history. The final
  hook-state amendment affects the hook acknowledgment only.
- The exact repository alignment test via recorded `cargo nextest run --locked -p farhelm-gui --lib -E <test>` (run
  `19b70985-514e-4783-9dd9-8be2be79e0b4`) showed the scrolling rows starting one pixel inside the painted border. The
  intermediate run `a52efe0e-2657-43d7-bbc3-9ece50aafebe` passed 187 of 188 cases and identified the Recent viewport
  measurement in the same bounds marker. The subsequent passing run covers both assertions.
- `cargo clippy --locked -p farhelm-gui -p farhelm-helm --lib --tests -- -D warnings` passed at `8b571d8c`. Cargo and
  Markdown formatting, changelog format, test-delay and whitespace checks passed. The instructions checker passed with a
  pre-existing archive warning.
- The final recorded `cargo nextest run --locked -p farhelm-gui --lib -E <selection>` passed 30 tests on `8b571d8c` (run
  `f5ae9e40-d1b6-4e32-a398-43e04f7b0eed`). This reran the actual folder edit during the setup write and the adjacent
  custom-registration checks after moving the save acknowledgment to its own state.
- Recorded `python3 scripts/check-gui-goldens.py --render-timeout 150 --scene <selection>` matched the published light
  and dark references for nine YOLO, wake, hook prompt, checkout and Recent scenes at `8b571d8c` (run
  `1280ecf6-8427-4664-be04-b829196f5566`). The new acknowledgment after a save is covered by the rendered GUI test.
- The delivery rebase added main's recipe-title and channel changes and the adjacent explanation of checkout preview
  waits. The stack's patches kept their behavior. The tested launcher tree and edited Restart routing still apply.
- The 16 registration and Recent references published during the first delivery still cover those rendering changes. The
  native macOS and full workspace suites were skipped in favor of the focused Linux GUI, helm routing and appearance
  checks for this round.

## Review gate

Independent GPT-6.2 Sol high source rechecks cleared the three revised PRs. The rechecks led to pinning every edited
Restart dispatch, keeping the successful hook-save status separate from other notices, and distinguishing panel and
viewport bounds in the tests. The registration layer has its original source review and an adjacent merge recheck.

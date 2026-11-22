# Replace with can use a later presented deletion guard after saving hook consent

Reviewed commit: bb31959c2f22c96bbbb72da2e60eee4a0b288a6b

## TLDR

Set up and launch can stop and delete a source that became live while hook settings were saving, without a new answer
from the user.

## Details

**highest.** Source inspection.

bb31959c2f22c96bbbb72da2e60eee4a0b288a6b crates/gui/farhelm-gui/src/launcher/view/hook_question.rs:164-205,229-251;
crates/gui/farhelm-gui/src/launcher/view.rs:1242-1256,1296-1316,1347-1357,2148-2155;
1a53d9d35889d7dde96e31320cc1681dd013737d crates/farhelm-helm/src/ui_channel.rs:990-996,1049-1068;
SPEC.md:2151-2168,2194-2219

Confirmed duplicate HSC-03; keep one highest finding. The new live state must be presented between the hook answer and
the client processing its successful settings reply. The ordered UI channel holds feed bumps during a command and sends
its projection before that command's reply; it does not continuously push during PUT. A push from a preceding queued
command or the PUT's own pre-reply projection can supply the new state, but the launcher must present it before
continuing. If the frame is not presented, no guard widening is proven. NothingAlive protects a new tab as well as a
restarted agent. AgentEnded protects a restarted agent while permitting tab closure. The continuation can substitute
AgentEnded for NothingAlive or Unconditional for either. The supervisor verifies actual running and uncertain evidence
under the lifecycle claim, rejecting before teardown for protected liveness (pinned
crates/farhelm-supervisor/src/service/handlers.rs:1244-1283; service/core.rs:13397-13503). Correct consent keeps the
original and reports both sessions if creation succeeded. The acceptance for additional tabs after a tabs warning and
leftover descendants after main-program exit is outside this defect. Confidence confirmed for the source ordering
conditional on presentation; its runtime frequency is unverified. Possible coverage: confirmed duplicate HSC-03.

Review evidence is retained under run `e551ad05-1866-44a9-b97b-9e47d1d67375` as `hook-launcher/HL-6`, with
`hook-supervisor-checks/HSC-03`, `hook-protocol/hook-answer-refreshes-source-consent`.

Update at `66ad3aaa95ef03a2130e2d0194d58353912ae9da` (commit `5fb16500b26ad95a1e1288756e46329440e17e84` and
`760b9be6eb5a036be863ccdaa21042f75eedb14e`): two later changes widen this finding. First, commit `760b9be6` (#4623)
removed the `if in_flight.is_none()` guard on `feed.changed()` in `ui_channel.rs:1058`, so the helm now pushes live
session updates continuously while `PUT /api/agent-hook-setup/{agent}` is in flight (removing the earlier caveat that
the ordered channel did not continuously push during `PUT`). Second, commit `5fb16500` promoted "Set up and launch" to
the primary confirm action in `prompt::ask_inline_fallible`
(`crates/gui/farhelm-gui/src/launcher/view/hook_question.rs:143-254`). A store push arriving just before activation
during GPUI's dirty redraw before dispatch also bypasses `Prompt::answer` because `hook_question` does not freeze or
refresh `SourceConsent` via `Inline::refresh_value`, and a push on the exact frame before `PUT` settles can instead
silently drop the launch at `launcher/view.rs:1306` after saving hook setup.

Extended under run `34570dac-b635-4682-a942-3a3fc1728e12` as `g25-restart-setup-and-launcher-wording/l1` (suggested
bucket: highest; confidence: confirmed).

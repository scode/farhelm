### What this was about

This round handled 32 November 24 review outcomes for Hosts, SSH and Tailscale, setup and updates, agent approvals,
credential delivery, and related desktop and CLI behavior. Twenty outcomes were fixed, five were closed without a change
under their triage gates, and seven became five follow-ups in TODO.md. Each outcome has its disposition and PR in the
archive. At delivery, the 14 PRs are drafts ready for the landing monitor to pick up.

### Things you should know

- Hosts actions now use the target and confirmation choice shown when you act. A settings or host change can make a
  press ineligible. It cannot turn a press on a displayed choice into approval of a different one. The risk also applied
  to Enter in the Add Host dialog, so both keyboard paths use the same presentation rule as the button.
- A refused setup can no longer write new paths into a retargeted host or a suspended sandbox. Remote Mac Update
  rechecks its intended target and refuses a newly detected desktop app before making changes.
- A later credential failure can leave earlier slots already delivered. Settings now reports a save error when that
  save's credential failed or never got its turn. Hosts still shows the later failure and any earlier pending cleanup.
- The stack touches neighboring plan areas where these outcomes required it. The environment overlap is setup admission,
  the deferred sandbox rerun, and the existing Stop-and-suspend caller of the shared Stop path. The Podman overlap is
  local transport error reporting at the same boundary as SSH. The launcher overlap is the checkout Save focus handoff.
  The Settings overlap is the credential receipt. The session UI overlap is the waiting Update question and its Send
  handoff. The agent approval work checks a changed host connection at dispatch and uses the existing workspace trust
  flow. The CLI help and the Update handoff rule are the specific wording outcomes assigned here. The deferred Wayland
  text paste needs a text-first path alongside the existing terminal attachment-first path.
- The rebase onto `eec1fdebe` preserved all 32 archive dispositions and integrated the intervening launcher, session and
  terminal work. The broader desktop run exposed an approval test that was sending raw keys before presenting the
  dialog. The fixture now presents it while Enter is still held. A repeated press must still do nothing, and a fresh
  Enter after release must send exactly one approval. The scope review also caught duplicate setup coverage. The final
  Hosts draft exercises both preference changes in the existing Add and Re-run tests.

### Open questions and possible follow-ups

- **Bind a confirmed Tailscale Uninstall to the verified peer.** The verified route needs to survive confirmation and
  removal as one operation. If the name resolves to a different peer after confirmation, a removal step can reach a
  different machine from the one the user approved.
- **Give explicit host actions priority over automatic SSH sign-in.** One shared sign-in answer can resume both an
  explicit action and an automatic check. The follow-up covers manual precedence, releasing a blocked route change, and
  a test with automatic updates enabled throughout.
- **Refuse setup reruns on suspended or deleting Tensorlake sandboxes.** The rerun needs to check eligibility before
  offering setup and again at confirmation, while preserving the old failed run and the connection hold. This is
  specifically the Re-run action. The ordinary SSH setup refusal already preserves the suspended sandbox and its paths.
- **Allocate Tailscale socket folders securely across helm restarts.** On a shared machine a preexisting folder can keep
  Farhelm from connecting to a Tailscale host. A private folder avoids that collision. The helm also needs to find and
  retire connections left by its previous process when the user changes or removes the host's route.
- **Make Wayland text-field paste nonblocking.** Text inputs need asynchronous text-first paste that keeps their normal
  edit and undo behavior. Terminals still need file and image paste to take priority.

### Pull requests

- **3 fixed, 1 closed, 4 deferred.** Keep local transport failures visible, scope the socket diagnostic to SSH, and
  preflight the remote Mac clock tool. Defer one outcome to Tailscale Uninstall peer binding and three to manual sign-in
  priority, including its regression test. Close the busy-retry test gap because exercising the real wake needs a
  separate contention fixture. [#4840](https://github.com/scode/farhelm/pull/4840/changes)
- **2 fixed.** Refuse setup before writing saved paths on a retargeted SSH host or a suspended sandbox.
  [#4842](https://github.com/scode/farhelm/pull/4842/changes)
- **1 deferred.** Move the suspended or deleting sandbox setup rerun to its TODO.
  [#4844](https://github.com/scode/farhelm/pull/4844/changes)
- **3 fixed.** Tie Re-run, Remove, and SSH setup confirmation to the action shown, including across sign-in.
  [#4849](https://github.com/scode/farhelm/pull/4849/changes)
- **1 fixed, 1 closed.** Distinguish same-named hosts by kind in the screen reader's Update label. Close the Tailscale
  checkbox focus outcome because it needs shared checkbox behavior in both editable and read-only states.
  [#4852](https://github.com/scode/farhelm/pull/4852/changes)
- **1 fixed, 1 deferred.** Preserve the Tailscale address format guidance in registration, edits, and startup. Defer the
  socket folder change. [#4854](https://github.com/scode/farhelm/pull/4854/changes)
- **1 closed.** Close the optional queued credential reconnect test without a change. Existing coverage exercises work
  already waiting on transport or a host lock. Testing work waiting to enter dispatch needs a larger fixture.
  [#4858](https://github.com/scode/farhelm/pull/4858/changes)
- **2 fixed.** Check that agent Create and Stop still target the approved host connection at dispatch.
  [#4860](https://github.com/scode/farhelm/pull/4860/changes)
- **2 fixed.** Explain that an agent can create a session in a new recipe environment or its own existing environment
  without an explicit host. Record that Send can open an already waiting automatic Update question when the existing
  keyboard focus rules allow it. [#4861](https://github.com/scode/farhelm/pull/4861/changes)
- **1 fixed.** Recheck remote Mac Update before any write if the supervisor cannot answer, covering an installed app and
  a changed target. [#4865](https://github.com/scode/farhelm/pull/4865/changes)
- **1 fixed.** Refresh launcher checkout availability after Save and keep focus where you moved it while the save was
  pending. [#4867](https://github.com/scode/farhelm/pull/4867/changes)
- **1 deferred.** Move nonblocking Wayland text-field paste to its TODO.
  [#4869](https://github.com/scode/farhelm/pull/4869/changes)
- **2 fixed.** Keep earlier cleanup warnings through later credential failures and report each save according to the
  selected credential's delivery result. [#4873](https://github.com/scode/farhelm/pull/4873/changes)
- **2 fixed, 2 closed.** Check both agents' entries in the shared-token fixture and rename the setup credential refusal
  test to match the notice it checks. Close the two optional developer-only diagnostics. Staged Update already tests
  manual-only admission. The additive host registration check now tests format errors and rollback for new rows in
  #4854. [#4876](https://github.com/scode/farhelm/pull/4876/changes)

### Checks run, reused and skipped

- On the rebase, Rust formatting, the Rust test delay scan, Markdown formatting, changed-line whitespace, changelog
  format, and instruction links were checked. The instruction checker reported an inherited archive reference to a
  renamed test-authoring heading.
- The rebased desktop selection covered Hosts, launcher checkout and view, session attention, approvals, and credentials
  in Settings. Run `7d59061c-2b35-4f3b-b416-04aa735578c8` passed 535 of 536. Run `a4e9d599-62ca-42b2-a48d-0b9962570312`
  reproduced the one approval fixture failure. The corrected fixture passed in isolation at `37e2cda1`
  (`ca1d32ae-0433-4773-b46a-96aacba6f9ca`).
- The full Linux screenshot comparison at `50bb5561` passed all four render jobs against the rebased manifest
  (`62ff9575-db6c-4317-9c81-83a02b1e4b5c`). The final fixture changes are in test-only code and preserve that drawing
  and manifest. The entire focused desktop selection then passed on final tip `29126774`
  (`d0173683-1882-4b43-a399-3fe3fe3f2da7`, 533 of 533, no runtime skips). It includes the revised Add and Re-run
  confirmation fixtures and the held-Enter check. The selection fell from 536 to 533 when two new preference cases moved
  into existing tests and a standalone Add presentation case was retired after passing. The presentation rule is still
  tested through Remove, Re-run and held Enter.
- Reused local evidence from the original reviewed drafts where the rebase preserved the code paths and fixture inputs.
  The recorder IDs identify the base and exact working tree fingerprint for each check.
  - Provisioning and setup admission, 13 cases (`cdc99633-4746-4179-a226-95575e73adb3`), plus 3 after refining the
    sandbox admission fixture (`0b918407-9a69-4fd1-a605-14b95de6fc10`).
  - SSH address validation and route checks, 10 cases (`d9e07ca6-34c8-484b-abb9-32f235bfbbcf`).
  - Agent connection checks, 10 cases (`b3b32cc6-c6d7-4420-adb7-6aa6a5684a3c`), and rendered CLI help
    (`fe4f4850-bbd2-47ab-93f4-935e543ac025`).
  - Mac Update admission, 4 cases (`ad1f6235-7c92-4e98-8788-ebe06988455a`).
  - Credential delivery and shared-token fixture checks, 35 and 34 cases (`ca1d8ef6-3c4f-4a4b-bef0-323e4a0cf444`,
    `f7761078-6b29-4509-9237-7d0a9c3f1a54`).
- The Mac Update admission tests used local stand-ins. Physical Mac setup and live Tailscale uninstall were outside this
  validation. Documentation-only and deferral drafts ran changed-file checks. The full workspace battery was skipped
  because the focused backend checks covered the affected commands and state transitions, and the rebased desktop checks
  covered the interactions with main.

### Review gate outcome

The original 14 drafts passed independent source and cold wording review. The final Hosts fixture consolidation was
reviewed again and cleared. The delivery report passed a cold read after the outcome counts, landing status and test
evidence were clarified. The visual review checked the affected desktop views in light and dark. The updated screenshot
references cover remote errors, setup refusal, SSH address guidance, Mac Update refusal, and combined credential
warnings. The consent and launcher focus checks used unchanged captures plus behavioral tests for the input sequencing.

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

### Landing

#### What landed with it

Only this plan landed this round, through its fourteen PRs and the landing fix
[#4882](https://github.com/scode/farhelm/pull/4882/changes). Between the point main was at when the executor finished
(`eec1fdeb`, which already held the launcher and terminal triage plans) and the landing, main gained only plans queue
commits, so nothing else could interact with this plan.

The landing's review read the whole combined diff against SPEC.md and SPEC_impl.md and found no bug in what it checked:
setup now takes the host's setup lock before it saves any install paths, and refuses a suspended host under that lock;
Update on a remote Mac checks again for the desktop app when it finds no supervisor; a local SSH or Podman client killed
by a signal now fails the step even when the remote side reported success; each credential save reports only the
failures that affected it, while Hosts keeps every warning; an agent's Stop and Create, and checkout creates and clones,
stay on the connection the user approved; and Add host, Re-run and Remove use the confirmation choice that was on
screen. The plan also changed how a Tailscale address the helm cannot use is refused; the review followed every route
that reports it (adding a host, editing its address, turning Tailscale on, setup and agent requests) and each still
answers it as the user's mistake, with the format guidance, not as an internal error.

#### Fixes made while landing

- **Nine screenshot scenes missing from the screenshot batch's scene list.** The batch that compares the desktop app
  with the old app's reference pictures keeps a list of every screenshot test scene, and its own test fails when one is
  missing. This plan added two scenes (a Mac Update refusal after the desktop app appeared, and the Tailscale address
  guidance in host settings) without listing them, and the launcher triage plan, which I landed two rounds ago, had
  added seven the same way. I did not run that test in that round, so it has failed on main since then. #4882 lists all
  nine.
- **A split SPEC.md paragraph.** When the plan added a sentence about the remote Mac's final setup check to the
  "Confirmed 2026-09-28" decision about how remote hosts get a supervisor, the rewrap left a blank line in the middle,
  so the rule that setup and Update may treat their installed layout as their own became a paragraph of its own and no
  longer read as part of that dated decision. No words were lost. #4882 removes the blank line.

#### Checks

Worker runs were on a separate arm64 Linux test worker; the rest ran locally on x86_64 Linux. All the checks below ran
on the plan's PRs rebased onto main as it stood at the start of the landing (`23324f7b`); the rebased stack's top commit
was `3d4e48e0`. The only later change is the landing fix [#4882](https://github.com/scode/farhelm/pull/4882/changes),
which adds nine entries to the screenshot scene list and rejoins one SPEC.md paragraph; after it I ran the scene list's
test and `dprint check` again, and both passed. That final tree is what main now holds outside the plans queue.

- The workspace Rust suite, `cargo nextest run --workspace --exclude farhelm-desktop`, on the test worker (recorder run
  `b19e646e-9baf-4173-bcfc-867820afae8e`): all 8653 tests passed, with no runtime skip. The plan changed the helm's
  provisioning, credential delivery, agent requests, host store and sessions, which the helm's own tests and the
  end-to-end tests both exercise, and the desktop app's Hosts views, so the whole suite was the narrowest run that
  covered them.
- `cargo nextest run --locked -p farhelm-desktop`, the desktop app's own tests, on the test worker (recorder run
  `ac495b45-e6d6-4a6d-8329-bd8751a68c69`): all 184 passed.
- `cargo clippy --all-targets -- -D warnings`, `cargo clippy -p farhelm --bins -- -D warnings`, and the desktop app's
  two clippy runs (`-p farhelm-desktop --all-targets`, with and without `--features agent`), on the test worker: passed.
- The desktop app's end-to-end smoke (`scripts/gui-e2e-smoke.py`, recorder run `6b706bfb-c22a-4e42-ab81-83e361389ce9`),
  on the test worker: 50 steps passed, one skipped, the paste step that headless Linux cannot run. It drives Hosts, Add
  host, the host menu's Remove, Update and Uninstall, and the agent approval cards, which the plan changed.
- `python3 scripts/check-gui-goldens.py`, the full comparison of the app's screenshot tests against their published
  reference pictures, locally (recorder run `7a95a315-33b7-491c-9691-c544ac125a7c`): every scene matched, the plan's new
  and changed Hosts scenes included.
- `python3 -B scripts/test-gui-screenshot-batch.py`, the screenshot batch's tests, which include a check that its scene
  list names every scene in the screenshot tests' manifest, locally: failed on the stack (recorder run
  `be915cfa-f195-4a8f-a18d-b624235e1902`), on nine unlisted scenes, two of them this plan's; passed with the landing fix
  (`d43bd3a1-0070-4f34-aa7a-58ec727af303`, 47 tests).
- `cargo fmt --all -- --check`, `python3 releasing/check-changelog.py format`, `python3 scripts/check-agents-md.py` (no
  errors; one warning, about an old citation in the triage archive, already on main), `dprint check` and
  `python -B scripts/check-test-sleeps.py`, locally: passed.

Reused: none. The report's own runs covered the same code before the rebase, and everything they covered ran again
above.

Skipped: `scripts/gui-lifecycle.py`, the end-to-end run that starts, quits and relaunches the desktop app, since nothing
in its startup, quit or window handling changed; the workspace doctests, since no documentation example changed; live
setup or Update against a real remote host or Mac, and a real Tailscale network, which no test in this repository
provides beyond the stand-ins the suites above use; and CI, which runs only on demand.

#### What the report says that is no longer true

Nothing became untrue at landing. One claim is broader than the code: the report says Hosts actions now use the target
and confirmation choice shown when you act, and one Remove button still does not (the second item below).

#### Things you should know that the report does not say

The landing's review counted no blockers, one should-fix (the first item below) and six small points, and judged the
stack sound to land.

- **The fix that keeps an approved agent Create on the approved connection has no test that would catch its removal.**
  When you allow an agent's request to create a session, the plan now refuses the create if the host's connection was
  replaced between your approval and the create being sent, rather than creating it over the new connection. The code
  does that, and the review traced it, but the two tests near it build the create by hand and never go through the
  approval, so removing the one-line fix would leave every test passing. The agent Stop request has a smaller gap of the
  same kind: a test shows Stop refuses a changed connection, but none shows the agent's Stop passes the card's
  connection along. The triage asked for a test. I did not write one at landing, since a test of that timing needs a
  fixture that replaces the connection mid-request and is more than a landing fix should take on. I recommend a
  follow-up that adds it; the review's sketch: a create that does not pin a trusted workspace folder, the connection
  replaced between the approval and the create being sent, expecting a refusal and no session on either connection.
- **The refusal dialog's Remove still reads the "Remove hosts without asking" setting at the click.** The plan fixed
  this for the host menu: its Remove now asks or skips the question as the screen said when the user pressed it. The
  Remove button in the dialog that explains why Update or Uninstall was refused still reads the setting when pressed, so
  if the setting is turned on elsewhere while that dialog is open (for example by "Remove, don't ask again" on another
  host), its Remove forgets the host without asking. It predates this plan and its triage outcome named only the menu. A
  follow-up would capture the choice when the dialog is drawn.
- **A refused setup confirmation now needs a new probe.** Setup confirmation now refuses at once while another operation
  holds the host (a retarget, a rename, Suspend or Resume, Remove), and that refusal discards the proposal the probe
  made (the install paths and steps shown for confirmation), so the user starts again from the probe. Before,
  confirmation could save its install paths while one of those operations was changing the host; SPEC_impl.md's rules
  for Update and Tensorlake reruns already refuse the same way. The code's own description of the confirmation step
  still says the proposal is kept for a later confirmation, which is now true only of the older "a setup is already
  running" refusal; that comment is worth correcting when the code is next touched.
- **Small wording, documentation and test points the review noticed, none needing action on its own.** SPEC.md's
  credential paragraph now says a save reports a failure "when delivery to a selected slot failed"; "slot" is never
  defined there. The code's description of how the helm sorts errors into the user's mistakes and internal failures does
  not mention the new Tailscale address check. The test of the Tailscale address guidance covers turning Tailscale on,
  though its name suggests it also covers adding a host and editing its address (they share the same error handling).
  The test that agent Stop refuses on a changed connection sits inside the test of an ordinary Stop.

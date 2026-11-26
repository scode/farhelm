# TODO

A running list of things the maintainer wants fixed or built. This is intent, not history: an entry is REMOVED in the
same PR that addresses it, so the file only ever describes what is still wanted. It is not a roadmap and carries no
priorities unless an entry says so itself.

Sixteen buckets, assigned by the maintainer: "definite simplification" is complexity the maintainer has decided to
remove — the decision is made, only the work remains; "planned" holds accepted work to implement later and suppresses
duplicate review triage within each item's stated scope; "plan decisions to revisit" holds the calls made while a plan
ran or landed, often on the maintainer's behalf when a plan blocked, each recording the alternative; "immediate" is what
should be planned right away, ahead of everything in "near term", and the todo planning flow selects its entries by
default; "plan follow-ups" holds the entries that came out of plans (follow-ups asked for in review, leftovers from
reports and landing notes), kept apart from the rest of the work; "near term" is what should be picked up next after
those; "linux support" holds Linux desktop app installation, packaging, and Linux-specific feature work; "tensorlake"
holds Tensorlake sandbox work, split into "v1" (the first non-experimental release) and "v2" (later); "mid term" is
wanted after the near-term work, sooner than "maybe later"; "doc todo" holds documentation work; "tricky bugs" retains
unresolved bug reports and their investigation findings; "deflake" gathers test and harness reliability work, including
CI execution and restoring gates; "broken tests" records tests that fail deterministically, with the failure and the
evidence that it predates any in-flight work; "code cleanup" holds structural code smells from a whole-codebase
assessment, grouped by kind with a suggested order; "maybe later" is wanted but not soon, and may never happen;
"unbucketized" is everything not yet sorted, which carries no implication either way. Within a bucket, no order unless
the bucket explicitly says so.

Known product fixes stay in their product bucket. "Difficult deflake" retains unresolved failures and their
investigation evidence within "Deflake"; that placement does not establish that the cause is test-only. Move a diagnosed
product fix out of "Deflake" rather than changing user-visible behavior as a test correction.

## Definite simplification

## Planned

## Plan decisions to revisit

- **Revisit Clone and Replace with starting old no-permission sessions as YOLO.** On 2026-11-23 the landing monitor,
  landing `remove-own-setting-permission` (#4669, #4672, #4673, fix #4685), accepted a consequence of migrating old
  agent sessions that never chose a permission to YOLO: Clone, Replace with and Restart with of such a session now
  preselect YOLO and pass the agent's bypass flags, where before they composed a Guarded launch for agents that offer
  it, and a Clone of one makes YOLO the remembered default. Hosts that ask before YOLO launches still ask and offer to
  start guarded instead; hosts set to start YOLO without asking start it silently. The release-notes draft says so. The
  alternative was to leave those sessions' stored permission empty and rely on the rule that already counts an empty
  permission as YOLO for confirmation, which keeps Clone and Replace with composing Guarded but departs from the
  decision to migrate them to YOLO. Plan: `plans/queue/remove-own-setting-permission.md`.

- **Revisit giving each repository cache its own directory path.** On 2026-11-23 the landing monitor answered
  `repo-cache-lock-redesign`'s block on the maintainer's behalf: recovery from a stuck repository cache gives each new
  cache its own directory, with a small Farhelm-owned record selecting the current one, plus a migration of existing
  caches and cleanup of an interrupted rebuild. The executor found that a Git process surviving an interrupted cleanup
  can reopen the cache's original absolute path, so recovering in place could let it touch the replacement. The
  alternative was to keep the current layout and defer checkout recovery, leaving sessions on an affected cache able to
  fail at checkout. Plan: `plans/queue/repo-cache-lock-redesign.md`.

## Immediate

- **Start several selected sessions at once.** When multiple sessions are selected we need to be able to start sessions
  if all the marked ones are stopped.

- **Right-click a session for its context menu.** Allow right clicking on sessions to bring up their context menu
  (instead of using the ... menu on the right). When right clicking and multiple sessions are selected, it should bring
  up the menu that applies to all selected sessions.

## Plan follow-ups

- **Name each custom-agent draft when closing Settings.** With unsaved changes on two custom-agent pages, or on Add
  custom agent and a registered agent, the Close question in `crates/gui/farhelm-gui/src/settings/editing.rs` says
  “Custom agent” once and can jump to a different page. Name every affected form. Give each registration a separate
  accessible row with its escaped display name and stable ID, and give Add and any other named forms accessible labels
  too, including when Add is the only draft. Use the question’s keyboard scroll stop for long lists. Stay on the current
  page when more than one section is affected, and preserve the existing behavior for distinct forms sharing
  Credentials. Save and Discard still apply to the versions of the drafts the question approved. Save waits for pending
  saves and leaves Settings open on the first refusal, with that form and its error visible. Test real changes to Add
  and a registered agent, the accessible labels, and the page retained behind the question. Capture and review the new
  prompt in light and dark, publish the accepted references, and include their manifest entries in the fix. See the
  Settings close contract in `SPEC.md`. The separate form names, keyboard access and their required evidence exceed this
  item’s size allowance. The triage decision asked only to reveal a page when one section is affected and to name each
  custom-agent draft in one sentence (size S); the separate rows, labels, scroll stop and new screenshots above are the
  plan executor's proposal, not a decision, and a smaller fix that meets the decision is fine.

- **Keep credential delivery reports visible when Settings closes.** A replacement saved through Save and Close can
  reach storage while some hosts miss delivery. `crates/gui/farhelm-gui/src/settings/credentials/editor.rs` sends the
  committed receipt and clears the editor. `settings/credentials.rs` puts the host names and reasons on the overview,
  but `settings/editing.rs` can close Settings before anyone sees them. Preserve the fresh delivery report and show it
  on the Credentials overview before closing. Continue other approved writes until the first refusal. A newer
  confirmation must keep its page and focus. If it delays revealing a report, or another refusal or dirty draft keeps
  Settings open, the next Close must give the person a chance to see the report. Once shown, retire the old automatic
  close while any remaining approved writes finish. Keep editing retires that close attempt, and an unrelated later Save
  must not inherit it. An older report that was shown must not hold a later Close open, and a replacement that committed
  must not be resent. See the Settings and Credentials paragraphs under “Session list” in `SPEC.md`. Test the exact
  committed PUT and host report, including a tunnel question opened before the receipt, then Cancel and Close. Review
  the Credentials overview in both appearances. The ordinary path, the deferred reveal after a newer action, and their
  required evidence together exceed this item's size allowance. The triage decision asked only to keep Settings open on
  Credentials when a close-save returns delivery warnings (size S); the rules above for a deferred reveal, retiring the
  old close and a later Save are the plan executor's proposal, not a decision.

- **Wait for credential saves after Back or Cancel.** A credential editor can start Save, then close while the helm
  still owns the write. Settings can close without waiting or offer Discard for another draft, and a refused write has
  no editor left to show its reason. In `crates/gui/farhelm-gui/src/settings/credentials/editor.rs`, Back and Cancel
  remove the editor through `Credentials::close_editor`; `settings/editing.rs` includes only the mounted editor and
  saves already owned by its batch. Track each admitted credential write by attempt after its editor leaves. Include all
  outstanding saves in the close question and join them on Save. On refusal, stop the close and show the reason in
  Credentials without replacing a newer editor or resending the write. Keep independently authorized saves and later
  close decisions separate as described in “Stop Settings Save and Close when an independent save fails”. Test Back and
  Cancel with a real pending request, both successful and refused replies, and another draft whose Save must wait and
  whose Discard must be absent. Assert that another editor's draft survives the refusal, and review the visible result
  in light and dark. See `SPEC.md` under “Settings” and “Credentials”. This needs receipt ownership outside the editor,
  integration with the multi-form close, and a new visible refusal with tests and references. The complete change
  exceeds this triage item's size allowance.

- **Show a failed hook setup save after Settings closes.** A person can turn off Set up conversation hooks on all hosts
  for Grok or Antigravity and close Settings before the helm answers. If the write fails, the previous consent can still
  apply to later launches and the reason is gone. `crates/gui/farhelm-gui/src/settings/editing.rs` inventories staged
  forms for Close. The immediate write in `SettingsDialog::toggle_configured` settles through `drafts::save`, whose
  callback only updates the original dialog. Keep the inline error when that dialog is open. If it has closed, use the
  same write's refusal to post an agent-specific notice with the reason and a way to retry in Settings. Reuse the window
  notice center and preserve the earlier choice on refusal. When a staged form brings up the close question, include the
  pending hook write in its pending state. If it refuses while that question or its Save and Close answer owns the
  write, stay on the agent's page with the reason and stop before sending more drafts. Leave a newer question or an
  independently authorized page Save alone. Use the same receipt for both paths and never send the hook write twice.
  Test the exact opt-out with plain Close before the reply, its success counterpart, and the refusal before and after
  accepting the close question with another draft staged. Review the notice and revealed refusal in light and dark. See
  `SPEC.md` under “Settings” and “Errors and diagnostics”. The receipt must outlive the dialog for plain Close and join
  the close decision for staged drafts. The triage assessment left the fix's shape open (hold the close, or a notice
  that survives it); choosing both, and an agent-specific notice with a retry route, is the plan executor's proposal,
  not a decision. Those paths and their test and visual evidence exceed this triage item's size allowance.

- **Reveal the credential editor when Save and Close fails.** Resolving a Tensorlake key conflict from the Credentials
  overview footer can leave that overview in front of a retained credential draft. Save and Close still submits the
  draft, but if the helm refuses the write, the overview offers only Resume editing credential. The reason stays in the
  hidden editor. In `crates/gui/farhelm-gui/src/settings/editing.rs`, use the failed editor's identity in
  `reveal_edit_failure` to clear `key_revealed` in `settings/credentials.rs` only for that editor. Keep its replacement
  secret and error in view as required by `SPEC.md` under “Settings” and “Credentials”. Confirm Save and Close while
  Resume editing credential is still shown in the overview. Check the exact credential PUT, refuse it, and assert that
  the same editor shows both the error and the retained draft. Capture the mounted error and staged draft in both
  appearances. The tested reveal, interaction test, and dedicated golden fixture together exceeded this triage item's
  size allowance. The separate independent-save follow-up covers a refusal that arrives while the close question is
  still open.

- **Stop Settings Save and Close when an independent save fails.** An editor can save on its own while a page Save waits
  for another write. If the editor fails before the page finishes, Save and Close must show its reason and stop before
  sending the remaining drafts. The frozen draft list prevents replaying the failed write, but does not show the refusal
  or stop other writes that the close answer queued. In `crates/gui/farhelm-gui/src/settings/editing.rs`, `ask_to_close`
  skips `watch_close_question` when a page batch exists. Keep the receipts joined for closing distinct from those the
  page Save already owns. Watch independent saves while the question is open and transfer them to the accepted close
  decision only if the user chooses Save. Opening another Close question gives up the previous close decision. A refusal
  dismisses the new question and shows the error if it is still open. Let the older page Save finish either way. Cover a
  refused editor save followed by an acknowledged checkout folder write. Confirmed close must show the refusal and send
  neither a retry nor the queued post-clone command. A newer unanswered or dismissed question must preserve the original
  page Save's post-clone write with its original draft. See `SPEC.md` under “Settings” and “One GUI at a time”. Deferred
  because separating receipt ownership and testing and capturing the revealed refusal exceeded this Settings triage
  item's size allowance.

- **Bind Settings footer Save and Discard to the page and drafts shown.** Navigation can move Save to another page's
  draft, including a different replacement for the one stored Tensorlake key. A late save refusal can move Discard to
  the page with the refused draft. In `settings/editing.rs`, bind `page_answers` to the presented page, form identities,
  draft revisions and available actions. Require the action to remain visible and eligible at keypress and across a
  pointer gesture. Search, an open Credentials editor or a busy form can change what the footer offers. GPUI can install
  a footer's handlers before presenting it and can carry a pending mouse-down across redraws. Capture the valid
  presented action on primary-down and require the same action at release. Keep keyboard activation at its normal press
  and preserve Close and the footer's focus and layout. Reuse the shared action frame and existing field revisions.
  Include the checkout field's loaded comparison base and cover the compound Notifications draft without copying its
  token. Test a held press across a page change and a down after the new footer is installed but before it is shown,
  followed by release after presentation. Cover a later same-page edit and an ordinary Save. The complete correction
  exceeded the triage size limit. See `SPEC.md` under “One GUI at a time” and `SPEC_impl.md` under “Questions: the
  prompt”.

- **Keep checkout notices after an agent Delete times out.** If an approved agent Delete outlasts the supervisor's reply
  budget and then leaves a checkout on disk, keep the checkout path and reason in the notices bell. The target host's
  result reaches `agent_delete` in `crates/farhelm-helm/src/agent_requests.rs`, while the asking connection's actual
  deadline and retained reply are in `crates/farhelm-supervisor/src/service/agent_relay.rs`. Under
  [Managed checkouts](SPEC.md#managed-checkouts), the helm must record the notice for the original target when the
  asking relay's answer budget has expired. Reuse the late Delete notice store. Correlate the deadline and result to
  that request on its own connection, including when the target is on a different host or in the asker's environment.
  Preserve the ordinary agent reply, retained Delete fence and existing timeout semantics. Exercise a real relay timeout
  followed by the matching checkout warning, and show that a timely reply still carries its notice. Reliable correlation
  spans both sides of the relay and exceeds this triage item's size gate.

- **Bind leftover sandbox consent to the saved Tensorlake key.** In Settings → Tensorlake, an open leftover sandbox
  Delete confirmation can survive clearing or replacing the API key in either key form. It can keep the old list on
  screen or send a Delete using a different key. Apply [Keyboard focus](SPEC.md#keyboard-focus)'s rule that a question
  closes when its subject is gone. In `settings/tensorlake_sandboxes.rs`, retire the question, old rows, and the refusal
  on key change. Reuse the account generation in `settings/tensorlake_key.rs` to reject consent from a previous key.
  Confirm must not queue a Delete behind a pending key write. A successful write without account data, or a server error
  after the helm may have stored the key, invalidates prior consent immediately. Re-read the account and require a fresh
  list before offering Delete again. Keep the draft and error on refusal, and show unfinished cleanup reported by the
  helm's next list. Test both key forms, a held write, a re-read after an ambiguous reply, and stale discovery. Verify
  the replacement view in light and dark. The ordering, refusal, test, and capture changes together exceeded the triage
  size gate.

- **Keep one-time YOLO confirmation through a host wake.** When a sandbox suspends or a container stops while “Start
  YOLO session anyway” is open, answering can wake the host and then ask the same question again. Capture the selected
  installation when New, Clone, Replace with, or Restart with sends the original launch request. Check that the question
  still applies before waking the host. Carry the one-time answer with the held launch through the wake and any checkout
  preview. Retain the original folder and source consent guards for Replace with and Restart with. Allow a fresh
  connection to the same verified installation. Clear the answer when that launch ends. Ordinary and guarded launches
  carry no override. Update SPEC_impl.md under “Questions: the prompt”. Exercise suspension between the question and
  answer, confirm the launch on the new connection, and show that changing installations before the refusal or during
  the wait, or starting a later launch, requires its own answer. Binding the original dispatch and testing the resumed
  launch across these paths exceeds the launcher triage budget. Plan:
  `plans/queue/launcher-consent-and-custom-agent-fixes.md`.

- **Report late wake failures when launching on a suspended sandbox or stopped container.** When Resume takes longer
  than the reply deadline, a later failure can leave the session launcher waiting for a minute before it gives the wrong
  reason. A slow Podman check can fail before Starting appears. The next host row can still say Stopped and carry the
  error from an earlier automatic start. Watching for a new phase or comparing error text cannot establish the outcome
  of the current request. Give the launcher the terminal result of the wake it is waiting on, including failure before
  the provider takes its connection hold. Start with the bounded reply in
  `crates/farhelm-helm/src/tensorlake/lifecycle.rs` and the existing launch wait in `launcher/launch/wake_state.rs`.
  Keep the 20-second reply deadline, the shared minute for a busy host and a fresh connection, and allow a successful
  Resume elsewhere to satisfy the wait. Restore the held launch draft on failure. Report a repeated diagnosis promptly
  if it belongs to this wake. Update SPEC.md's launcher wake contract and SPEC_impl.md under “The UI channel” for this
  launcher-specific result. A failed start or a container that stops again should use Starting or Stopped wording, per
  [Container sessions](SPEC.md#container-sessions). Exercise a late failure before Starting, repeated failures for both
  providers, and a direct start refusal. Verify the launcher alert in both appearances. The fix needs a result the helm
  can attribute to the bounded request, beyond the launcher-only triage budget.

- **Require the fresh-clone warning before Replace with can act.** In Replace with, a recent setup can switch the
  destination from the source's managed checkout to a fresh clone. On a suspended host, the same Enter can start waking
  the host. On a connected host, a second press can launch once the preview returns while the old frame still shows the
  source folder. The replacement leaves the source's changes behind and can move its checkout to Trash. Bind the launch
  to the checkout consequence actually shown. A warning laid out inside the scrollable form can be clipped, and GPUI can
  replace a visible draw before presenting it. The triage attempt tracked prepaint visibility but the next-frame
  callback could admit it before the clipped replacement frame. Establish actual presentation or keep the disclosure
  visible through the press. Bind it to the current checkout and carry the admitted consent through hook settings, host
  wake and checkout preview. A newer frame with an unrelated change to the source's tab count should not cancel a launch
  whose consent already covered it. A changed checkout requires new consent. See `SPEC.md` under “Managed checkouts” and
  “One GUI at a time” and `SPEC_impl.md` under “Questions: the prompt”. Test both recent-setup paths, a visible draw
  replaced by a clipped frame before presentation, and a successful continuation after a wait. Update the documented
  Enter behavior and key hint if the fill now needs another press.

- **Keep wrapper defaults per custom agent registration.** Farhelm remembers one wrapper for all custom agents. A
  launcher switch or template can carry an explicit prefix from one agent into another, and launching a second agent
  without a prefix can erase the first agent's default. When a user agent launch succeeds, update only that
  registration's wrapper memory, clearing it for an unwrapped launch. Ignore the old shared Custom default because its
  owner cannot be recovered. Reload the destination agent's default and clear the previous explicit choice on a chip,
  search or template switch. A template's explicit prefix or clear overrides the remembered default. Use the existing
  switch notice when a deliberately chosen model or effort is cleared. Preserve saved launch commands and built-in
  defaults. Prune only that registration's memory in the Delete transaction and prevent an in-flight launch from putting
  it back after deletion or re-registration of the same ID. Update the creation rules in SPEC.md and the preferences
  contract in SPEC_impl.md. Cover two registrations, blank and explicit prefixes, template replay, upgrades and
  re-registration. The triage plan deferred the combined change because it spans stored and pushed preferences, launch
  history, deletion and both launcher paths.

- **Publish Restart with settings with the replacement generation.** A custom agent can report a conversation as soon as
  it starts. Restart with currently publishes its new generation before saving the edited launch in the supervisor
  store, so startup reports can be accepted or permanently refused under the previous reporting setting. The reply and
  concurrent listings can also give the old Restart offer. Bind the new launch to the generation before its reports can
  be admitted, then publish the matching snapshot and derive the offer from it. Preserve the old launch on a definitive
  abort and retain the new settings when the process starts but building the reply fails. This spans the generation
  transaction, recovery and terminal publication, above the triage size gate. Test startup reports with reporting
  switched in both directions, the offer after a named restart, and the persisted launch on each failure side of spawn.

- **Use the shared Delete confirmation for custom agents in Settings.** The footer asks with two ordinary buttons.
  Escape can close Settings instead of cancelling Delete, focus stays away from Cancel, and the question never names the
  registration. Port `settings/custom_agents.rs` to the recipe editor's inline prompt as described in
  [Questions: the prompt](SPEC_impl.md#questions-the-prompt). Freeze the name and stable registration ID, start on
  Cancel, and keep the Delete button's focus handle alive so Escape can return to it after the footer redraws. If Delete
  fails to start, keep the draft and show the reason without replacing the text of an open question. An unanswered
  Delete must also let an accepted Save and Close proceed, or show why the save could not start. Coordinate that part
  with the Close → Save outcome in `triage-settings-nov24`. The prototype passed the screenshot and cancellation checks
  but exceeded the triage budget before it covered focus restoration and the silent Save refusal. Test both from a dirty
  registration and check light and dark captures.

- **Distinguish unread custom agent registrations from deletion.** When the session launcher opens a saved custom agent
  choice before its list arrives or after the helm fails to read it, it can wrongly say the registration was deleted.
  Keep the choice visible on both tabs and keep Launch unavailable when there is no entry to use. Show whether the list
  is still loading or the read failed. Keep using entries retained from a previous successful read, and show deletion
  advice only after a successful list omits the ID. Apply the same distinction to a stale chip or search choice and the
  Command tab's retained choice. Restart with must keep its required conversation field visible when the list is loading
  or unreadable, even if the field is empty. Show the reason by the field. Keep loading text neutral and avoid guessing
  whether resumption or reporting is supported while the entry is unknown. Check both tabs in all three list states,
  with light and dark screenshots. Before pressing Launch, confirm that the fixture installed the intended catalog state
  and that the saved ID is absent. The prototype exceeded the triage size gate before it handled the empty Restart with
  field and the loading message beside Launch. Plan: `plans/queue/launcher-consent-and-custom-agent-fixes.md`.

- **Show Custom approval flags in keyboard-highlighted Recent setups.** Two Recent setups in the launcher can show the
  same agent and Custom approval mode while saving different flags. Pointer hover and the screen-reader label give the
  full choice. When the command field keeps keyboard focus, show the saved flag words on the highlighted row in both the
  command pane and narrow search results, before Enter. Keep the compact wrapper summary and avoid adding a Tab stop. A
  prototype that wrapped the flags in place still clipped the last words when a highlighted row was taller than the
  results area. Give keyboard users a way to read those words without changing the highlight, and test two setups with a
  long shared flag prefix and different final arguments. The complete fix exceeds the triage size gate. Plan:
  `plans/queue/launcher-consent-and-custom-agent-fixes.md`.

- **Make Wayland text-field paste nonblocking.** Pasting into a text field such as an SSH sign-in answer can freeze the
  window while another app supplies clipboard data. [Terminal experience](SPEC.md#terminal-experience) requires Farhelm
  to stay responsive during clipboard operations on Wayland. Kit's Paste checks the synchronous read before its async
  fallback. Give that path an async text-first read on Wayland, reusing Kit's focus, document and selection checks and
  its atomic undo. Keep the terminal's file-first paste and keep text-field newline handling. The vendored async path
  currently prefers files and images, so sending both callers through it can lose text from a mixed offer. The split
  spans the Wayland receive code and the clipboard API used by Kit and the terminal. Cover keyboard and Edit menu paste,
  mixed offers and a reply that arrives after the target changes.

- **Show container recipe admission failures before recording build ownership.** An explicit Rebuild can answer Accepted
  before a fresh Podman check fails. The existing recipe place record keeps that failure visible after the job ends.
  Suppressing the record alone loses the reason if the old host check still says Ready, for example after a refresh
  times out. Return the refusal before acceptance or keep it in the existing build status without creating a recipe
  place entry. Fence the first recipe place write to the host settings and machine/account claim that passed admission.
  Looking up the host again by row ID can use different settings after a retarget or an SSH mode change. Cover a first
  Rebuild that fails admission and still shows the reason, plus a route change between admission and the first write.
  Recipe Delete now handles historical records with no resources. Preventing those records at the admission boundary
  also needs a way to show the outcome of an already accepted Rebuild.

  Recheck admission when launch preparation or Rebuild starts, before each new helper's base pull, and before each
  helper create. Dispatch from the route and Podman executable that passed the final check. A separate runtime lookup
  can wait on another probe and accept a capable engine even after the ordinary Farhelm connection refuses new work.
  Keep that runtime path for existing resources. Check for cancellation when the admission and pull waits settle. Clear
  the helper's name and start marker if admission refuses or cancellation arrives before the create is sent. Keep the
  existing recovery rules if a create might have been sent. Test a refusal while the final route is resolving and one
  after a successful pull. Confirm both leave no create and clear the reservation, and cleanup of older work can still
  run. See “Verify container hosts before trusting results or sending private input” for the shared requirements on
  connection proof and cached observations.

- **Verify container hosts before trusting results or sending private input.** Each provider command for a claimed
  container host must verify the recorded machine and account on the connection that will run it, before executing or
  accepting an engine result. Use a proof tied to that authenticated endpoint. A token sent with the command does not
  establish who answered. Accept capabilities and claims only from the verified machine and account. The probe collects
  capabilities over several commands, so verifying its last reply alone cannot establish where the earlier replies came
  from. Verify those observations before accepting a newly discovered place's first claim. Apply the same check to
  recipe scripts, the creation-time proxy read and delivery of the container's proxy file. Hold private input until
  verification succeeds on the connection that will receive it. A separate preflight can succeed while a later
  connection reaches a different account. The existing command runner sends private input immediately and exposes the
  final output after the process finishes. The full fix needs a transport boundary that verifies before sending bytes
  while preserving cleanup on timeout and cancellation. Leftover discovery and Delete must carry any existing claim
  across every command, including absence checks. When the runtime has no cached host observation, its probe can replace
  the claim before cleanup reads it because a leftover has no registry row. Preserve the cleanup identity across that
  refresh and retry, while keeping explicit route changes and adoption consistent. A check that refuses a moved place
  can currently replace the runtime's last matching Podman path and proxy settings. Keep that matching observation
  available to existing resources and recheck when the cache has no observation for their recorded machine and account.
  Keep the refusal visible to new launches and builds. This includes a cold cache after Adopt when existing resources
  still need their claimed machine. Only retire an unused claim after an ordinary supervisor refusal when the place
  check establishes that the endpoint moved. Use a claim established by the first probe if one becomes available, and
  preserve the existing ownership checks for legacy rows without a claim. Update the container-session contract and
  cover withheld private input, a cold-cache refusal, a refused retry and successful discovery or cleanup after the
  original destination is restored.

- **Keep host proxy settings out of Podman recipe images.** Recipe build and preparation helpers write
  `90-farhelm-proxy.conf` for their systemd user managers. A captured image can carry that file, including passwords, in
  the host account's Podman storage. A new container can then keep a proxy the host removed, because systemd read the
  image's file at boot before Farhelm imports the current settings. Remove the exact file before each software and
  prepared image capture and require removal to succeed on the verified place. Use an explicit working directory the
  recipe login can enter. A prepared image made from an older software image would still carry that image's layers even
  after the file is removed from the helper. A same-version prepared image can also hit the cache directly. Track
  proxy-free capture in image provenance and replace ineligible software and prepared images through the existing
  publication and retirement paths. Check eligibility when a recipe names a Farhelm-built software image as its base as
  well. Keep readiness in Settings aligned with launch eligibility, and preserve recorded build inputs and existing
  sessions during the transition. Cover legacy-cache replacement, refusal before capture and the exact boot file for a
  new container, including a removed key and a different key set to empty. The capture and boot-file changes alone
  exceeded the triage size target, and the cache transition needs its own coverage to preserve working recipes.

- **Handle programs that outlive their terminal host.** The desktop can describe a session as ended while its main
  program still runs. Stop or Delete can then exceed what the user confirmed. On an unscoped launch, Stop or Restart can
  miss the survivor if it cannot find a session marker. Count a surviving main program as alive for consent and root
  authorized Stop and Restart at its retained PID and start identity, checked against the process table as Delete
  already does. Refuse Stop, Delete and Restart on a process-table read error, and refuse ended-only consent while the
  program is alive. Keep terminal availability separate so recovery stays available and the desktop can ask for the
  consent it actually needs. Use current evidence for the recovery Restart decision as well as for ordinary Stop and
  Delete. The ticker must wait for evidence of exit or successful cleanup before it records a completed Stop. The triage
  experiment fixed only the supervisor's action checks, which left desktop retries repeating the wrong consent and let
  an old activity sample block recovery Restart. The full fix needs shared evidence for status, confirmations and
  cleanup, with tests covering those boundaries.
- **Settle an unconfirmed agent start before Stop reports success.** A launch can stay pending with no terminal when
  Farhelm cannot confirm its terminal host ended. Authorized Stop can record an exit and return success without checking
  the recorded start. A first stage that still holds the launch spec can start the agent after that reply. Use Restart's
  recorded-start settlement under the lifecycle claim and after both consent guards. Include pinned agent children in
  cleanup and keep the launch evidence if the recorded scope or process sweep is unconfirmed. Preserve the distinction
  between a launch that left no live agent and a Stop that found one. The latter needs the user-stop annotation even if
  a sweep retry no longer sees the child, and an outcome write failure must report that cleanup already ran. A direct
  early sweep missed both cases. The final fix needs a durable stop-intent and retry path beyond the triage budget.
  Extend the retained-create fixture in `host_creates.rs` to drive authorized Stop against an unanswering and a gone
  host, and add an answering host with a live child. Assert refusal preserves the launch, a completed retry preserves
  its stop annotation, and a failed write reports the work already done. The existing durable Stop intent treats a
  missing terminal as exit evidence. Same-boot recovery can also clear the intent when it rediscovers a live host, or
  complete it from a dead child even though the scope has not been checked. Carry the pending sweep through observation,
  status and consent, and retries after adoption. An empty terminal address also occurs when create confirmation fails
  after the terminal became usable, so changing that case's recovery needs an explicit decision. A keyed create retry
  already preserves an existing Stop intent without starting another agent.

- **Keep a failed tab close retryable after the terminal host ends.** Close can end the terminal host and then fail to
  confirm the final sweep for surviving shell processes. It returns an error after forgetting the tab and removing its
  launch record, so Close and the automatic cleanup cannot retry it. If the host end also fails, the error claims the
  processes were reaped even when their sweep failed. Keep the cleanup evidence until both checks pass, as SPEC.md's
  “Lifecycle operations” requires. Start with `close_host_tab` and the tab reap's existing backoff. Retaining the record
  in Close alone leaves a gap after a normal supervisor restart. Reload drops tabs whose host is gone and startup
  reclaim removes their records. The launch record also lacks the scope selection that the ended host's tag supplied.
  Carry that selection and the pending cleanup across reload without re-adopting a closed tab or sweeping unrelated
  tabs. This needs a durable recovery design beyond the triage budget. Exercise a successful host end with an injected
  final sweep failure, a failure of both checks, and a retry across supervisor restart with the scope initially
  unavailable and then verified gone. A pending close's first sweep can fail before the failure branch that advances the
  retry deadline. Advance the deadline on every failed automatic retry and assert that cleanup waits until the new
  deadline.

- **Keep hook setup from launching into a folder that arrived later.** In the launcher, pick Grok or Antigravity to open
  the Restart hook question. Switch to a connected host whose remembered folder is still loading and choose **Set up and
  launch**. A history reply can settle the folder while the all-hosts setting saves. The continuation then launches in a
  folder the question had yet to show. Bind the continuation in `launcher/view/hook_question.rs` to the readiness,
  destination and source warning shown with the answer, per SPEC.md's “One GUI at a time”. Save the setting and leave
  launch to a new press when the shown launch was refused or its destination changed. An unchanged sleeping host can
  legitimately wake and prepare a folder or checkout, so a missing launch request is insufficient to tell which answer
  was safe. Wake can take its folder from a newly selected session before the next frame. Settle that default before
  comparing. For disposable environments, compare the actual placement and ignore any retained host choice from the
  ordinary launcher. The fix and test exceed the triage size gate once they cover these paths. Reproduce from New with
  no open session and Folder untouched. Use the real host picker and hold the new host's history reply until the setting
  write starts. Draw the settled folder before replying to the write. Verify the write succeeds and only a fresh press
  starts the session in the settled folder. Plan: `plans/queue/launcher-consent-and-custom-agent-fixes.md`.

- **Ignore Enter on a clipped recent setup.** In the session launcher's empty command pane, highlight a recent setup
  with the arrow keys and scroll it out of view. Enter can still launch that setup or replace the source session. Keep
  the highlight and consume Enter until a frame has shown the whole row again, as SPEC.md's “One GUI at a time”
  requires. In the launcher search view (`launcher/view/search.rs` and `launcher/view/search/start.rs`), extend the
  existing Enter action snapshot with the row's visibility after layout and scroll clipping, including the ancestor
  clip. Invalidate it when the row disappears or is clipped. The next visible frame authorizes a fresh press. Preserve
  the existing stale-choice refusal if the setup disappears before dispatch. Test partial and full clipping by scrolling
  a selected row, then reveal it and verify the next Enter sends the stored setup's distinct model. The fix exceeds the
  triage size gate because the existing snapshot is taken before layout, and the regression needs a scrollable pane and
  a real Replace request.

- **Stop the agent updater's process group before running drift cases.** `scripts/harness_upgrades.py` currently times
  out the top-level process. An updater or fallback installer child can still replace the CLI after the runner probes
  its version, so a case can report a verdict for the wrong binary. Cover both timeouts and cancellation during
  preparation, including a signal arriving as the process starts. Reuse the same teardown for the updater and the
  fallback shell, and verify with a local child process that no write can race the version probe or a case. The first
  implementation could kill a worker still finishing a successful update. Waiting instead with a process listing could
  miss a successor forked during the scan. The next design needs to confirm the group has finished with kernel-backed
  evidence and keep its identity safe to signal through timeout and cancellation.

- **Finish Cursor launch-prompt verification.** Cursor Agent 2026.11.22-e9042b2 showed “Press any key to log in...” when
  started with `cursor-agent -- "$prompt"`, where the prompt starts with `--` and includes a newline. Run
  `python3 -B scripts/run-harness-drift.py --no-upgrade --category initial_prompt --harness cursor` with an existing
  Cursor login. Verify the fresh turn, then give the case a conversation ID it can resume with
  `cursor-agent --resume "$id" -- "$prompt"`: Cursor's resume list is interactive, but `cursor-agent create-chat`
  returns a new chat ID before the first turn. As it stands, the named step can only report COULD NOT RUN for Cursor.

- **Verify launch prompts when reopening OpenCode conversations.** OpenCode 1.19.6 answered the fresh launch prompt.
  `opencode --session "$id" "--prompt=$prompt"` reopened that conversation and showed the earlier reply and an empty
  composer. It never showed the recall prompt or its reply. Reproduce with
  `python3 -B scripts/run-harness-drift.py --no-upgrade --category initial_prompt --harness opencode`. The tested
  release's TUI opens the named session without passing along the startup prompt. Check a release that includes the
  upstream fix.
  [OpenCode 1.19.6 startup code](https://github.com/anomalyco/opencode/blob/v1.19.6/packages/tui/src/app.tsx#L449-L469)

- **Verify OpenRouter reasoning effort with credentials.** The new reasoning effort checks for Goose 1.60.2, Pi 1.6.0
  and OMP 19.1.1 stopped before launch because `OPENROUTER_API_KEY` was missing. Run
  `python3 -B scripts/run-harness-drift.py --no-upgrade --category reasoning_effort` with the key to exercise Goose's
  `GOOSE_THINKING_EFFORT` and the two `--thinking` flags. Confirm that Pi and OMP display each requested effort and that
  at least one enabled effort per CLI completes a turn. Goose 1.60.2 shows the chosen provider and model in its startup
  banner and status, so its terminal can confirm the launch and reply. Verifying the effective provider effort for Goose
  needs other evidence.

- **Finish live Cursor model and permission verification.** Cursor Agent 2026.11.22-e9042b2 showed “Press any key to log
  in...” with both `--model auto` and `--model composer-2.5`. Reproduce with
  `python3 -B scripts/run-harness-drift.py --no-upgrade --category model_selection --harness cursor`. Run with an
  existing Cursor login and identify the active model field. The older authenticated screen fixture displays a bare
  label such as `GPT-5.6 Luna 272K Low` beside the composer. With the same login, run
  `python3 -B scripts/run-harness-drift.py --no-upgrade --category permission_modes --harness cursor` to confirm YOLO
  and Custom launches. Both of those starts also reached the login screen. The `named_conversation` category's Cursor
  case, which seeds a chat from `cursor-agent create-chat` and reopens it by ID, also stopped at the login screen and
  still needs a run with the same login.

- **Finish OpenRouter and saved-default model verification.** Goose 1.60.2, Pi 1.6.0 and OMP 19.1.1 opened with an
  active model when started without `--provider` or `--model`. Compare the active choice with the saved provider and
  model to catch silent fallbacks. Run the named cases with `OPENROUTER_API_KEY` and verify both fields for
  `--provider openrouter --model <id>`. Pi 1.6.0 can omit the provider in its footer, including on an OpenRouter-only
  setup, so that case may need a provider-bearing readout.

- **Review the two-lane desktop channel before moving more commands.** Check how the shared queue limit, per-lane
  watchdog and operation keys interact, and verify the boundaries in `Work::lane()`. Connecting N hosts and opening
  Checkout Trash can send up to 2N reads in quick succession. Revisit the choices to stop rechecking after a failed read
  and wait for the pushed epoch after a delete times out. If a post-delete read reports that removal is finished,
  another read is pending when the old delete reply arrives, and that pending read fails, the can can still show removal
  until the next refresh. If the first refresh fails before the delete reply arrives, Trash may show the read failure
  without showing the accepted removal.

## Near term

- **Move agent records off the host.** Implement the design in
  https://snippets.scode.org/s/farhelm-records-on-artifacts-sketch/ (goals, decisions and the reasons for them are on
  the page), so plan executors and the landing monitor can run on other hosts: their logs and notes go to one Cloudflare
  Artifacts repo behind a small records tool, and plan state stays on GitHub main. Start once the plans in flight have
  settled.

- **A slow host still delays ordered desktop actions.** The desktop app keeps live updates flowing during host waits.
  Session Stop and Delete, environment Suspend, Resume, Stop and suspend, Delete and Retry-delete, and Checkout Trash
  listing and deletion now run independently of ordered commands. Credential replacement is the next candidate. It waits
  at most 20 seconds total for delivery across hosts, with up to four deliveries in parallel. The remaining ordered work
  includes launcher reads and Launch, session Rename, Restart, Replace, tabs and tunnels, and host management and
  settings, including Delete sandbox or container from the Hosts panel. Review the two-lane channel and the operation
  boundaries before converting more of them. Plan: `plans/queue/slow-host-async-actions.md`.

- **Idle CPU of the GPUI desktop app.** Accepted for now on 2026-11-16: an idle window with one running session uses
  about 3.05-3.09% CPU on the test Mac (Apple silicon), against 1.61-1.62% for the last browser-UI build in the same
  session. Frames are back to the old app's rate (3.0 changed frames a second, after the cursor blink was aligned with
  the running pulse); the whole gap is cost per frame, about 4.4 ms against 1.8 ms, spread over the session list (+0.86
  ms), the session header and tabs (+0.76 ms), whole-tree layout (+0.74 ms) and the shared buttons. Most of it is GPUI's
  own per-element work. Levers looked at and not taken: lighter Button and IconButton (about 1% of a frame), repainting
  only the pulsing dot (a vendored GPUI patch; helps only pulse frames), and caching the session list and header behind
  the cache wrapper, the one large lever, which drops a cached view's screen-reader information unless GPUI is patched
  to replay it. Look into caching with that patch, or another way to cut per-element cost.

- **Recognize Muse Code questions and approvals.** Detect questions and approval dialogs during a conversation from real
  Muse screens. These can currently look idle while Muse waits for an answer. Capturing them needs an authenticated test
  provider or a documented provider fixture protocol that can ask a real question. Plan:
  `plans/queue/muse-session-tracking.md`.

- **Run the desktop app's own supervisor as the same launchd job remote Macs get.** Asked for by the maintainer on
  2026-11-08 while planning remote macOS hosts: once that plan has landed and remote Macs are validated as stable, move
  the local Mac's supervisor from being started detached by the desktop app to the same per-user launchd job, so every
  Mac uses one consistent mechanism. Related: "Let the Mac's supervisor outlive the desktop app" under Maybe later.

- **Decide how terminal snapshots are supposed to work.** Every checkpoint of a terminal in a terminal host encodes the
  whole terminal (screen, up to 12,000 lines of scrollback, modes) into one self-contained blob, even though most of the
  scrollback has not changed since the previous snapshot. That repeated work is what makes typing pause during a
  checkpoint and what makes the burst at an orderly supervisor stop expensive. The plans `faster-snapshot-encoder` and
  `stop-checkpoint-burst` only make today's whole-terminal encode cheaper and rarer. The maintainer takes a position
  first, in particular on whether snapshots should be maintained incrementally, and implementation follows. Two shapes
  came up while planning those plans: incremental on the host (a base snapshot plus a chain of changes, which needs a
  new snapshot format and a protocol change for hosts that outlive every release), or incremental in the supervisor
  (cache the encoded bytes of history rows and re-encode only the screen and new rows, producing the same v1 blob as
  today). The repository records no reason for the original choice of whole snapshots.

- **Credentials for ordinary SSH hosts.** The Credentials section of Settings serves environments through their recipes.
  Extend it so ordinary SSH hosts can use the same named credentials.

- **Cut the new desktop app's idle CPU with a large running fleet.** With the session list now drawing only the rows on
  screen, idle CPU no longer depends on how many sessions are listed, and typing stays at about 30 ms at any fleet size.
  But it still grows with how many sessions are running: on the test Mac, 1.6% with 5 sessions, 3.3-3.5% with 179 of 200
  running and 5.3-5.4% with 449 of 500 running, against the old app's 1.6% idle. Most of it remains when the window is
  in the background, so it comes from applying the helm's updates for running sessions, not from drawing. Asked for on
  2026-11-14; first measure the old app with 200 running sessions for a fair comparison, then profile what each update
  costs in the app.

- **Remove the "Remove the bundled tmux" step from Add and Update.** Since tmux-drop-from-install (#3371 to #3374),
  every Add and Update plan for a remote Linux host includes a step that deletes the tmux copy earlier releases
  installed beside Farhelm. It is needed while Update can reach a host still carrying that copy: any release from 0.23.0
  (the current upgrade floor) up to the one before tmux was dropped may have installed it. Remove the step, its
  explanation, docs and tests once the upgrade floor passes the release that dropped tmux, or together with "phase 3"
  (Mid term), whichever comes first; a host updated after the step is gone keeps an unused copy, which is harmless.
  Decide the timing when this is picked up. Recorded by the maintainer on 2026-11-18 from the plan's review.

- **Finish the tmux cleanup: the known leftovers and one final sweep.** tmux-drop-from-docs (#3381 to #3385) audited
  every remaining tmux and xterm.js mention and left these, each with the plan's recommendation:
  - The 65-second "agent has not reported its identity" notice skips Enters inside a paste by scanning typed input for
    xterm.js-style paste markers. The desktop app sends pastes as separate messages, so the scan probably never matches
    now; confirm and remove it.
  - Delete runs an extra scan of every process on the host for every session that was ever restarted, because an earlier
    release's Restart could leave tmux tabs nothing lists; its cost was never measured. Goes with "phase 3".
  - The per-session terminal-kind tracking and the paths that read it, test fixtures that still create old-kind rows by
    default, and about 3,000 lines of test tooling for real tmux servers that now serves only the startup step's four
    tests. Goes with "phase 3".
  - Test-only: a half-second pause before an agent exits in the session lifecycle tests, and an extra error an
    Attach-racing-Delete test still accepts; both existed for tmux, and nobody has checked whether terminal hosts need
    them. Check and remove with "phase 3".
  - Renames when the code is next touched: the "pane" vocabulary across the supervisor and tests,
    `PRIVATE_TMUX_ENV_VARS` (now about the user's own tmux), and test names saying "new terminals" or "never in tmux".
  - Left as they are by the plan: status detection's 16-screen read budget per round and the 10-second prompt delivery
    timeout, both sized for tmux; revisit only if a better value appears.

  Then sweep the repository once more for tmux, xterm.js, pane and old-terminal mentions and remove or reword whatever
  is left that does not belong to "phase 3". Recorded by the maintainer on 2026-11-18 from the plan's review.

### Drift test feature coverage

- **Drift tests: base launch, Codex process isolation, and Cursor/OpenCode interactive turns.** Verify that Codex's
  `--no-daemon` keeps hooks and commands inside the session's process tree and still launches cleanly when hooks are
  disabled, verify Cursor's default launch without a permission flag ("Agent's own setting"), and send a real prompt to
  Cursor and OpenCode to confirm interactive turns work under Farhelm's terminal host (today both stop at the initial
  TUI or trust screen).

- **Drift tests: opt-in persistent hook installation for Grok and Antigravity.** Using isolated config directories so
  the real user home stays untouched, verify that the exact hook files Farhelm installs for Grok (`hooks/farhelm.json`)
  and Antigravity (`plugins/farhelm/{plugin.json,hooks.json}`) are discovered and invoked by the real CLIs inside
  Farhelm to capture conversations, and exit cleanly and silently when the CLIs run outside Farhelm.

- **Drift tests: in-session conversation rotation.** Wire the existing standalone Claude `/clear`, Codex `/new`, and
  Codex `/clear` checks into the drift runner (running Codex `/clear` end-to-end through Farhelm's own hook so pending
  withdrawal and `Stop` confirmation are tested), and add live rotation coverage for Muse (`/clear`, `/new`), Pi
  (`/new`, `/fork`, `/resume`), OMP (`/new`, `/fork`, `/resume`), Grok (`/new`), and Antigravity, proving Restart
  resumes the rotated conversation.

- **Drift tests: subagent and nested-agent report rejection.** Wire Claude's standalone shelled-out `claude -p` test
  into the drift runner and add live coverage for both native subagents and shelled-out nested agents across all six
  foreground-ownership harnesses (Claude, Codex, Muse, OMP, Grok, Antigravity), proving child invocations cannot replace
  or corrupt the foreground session's conversation ID.

- **Drift tests: terminal screen status and dialog detection.** Verify live anchored `Running` and `Idle` readings on
  Claude, Codex, and Muse (catching silent fallback to generic change-counting when vendor TUI layout drifts), live
  workspace-trust prompt detection on an untrusted directory for Claude, Codex, Muse, and Antigravity, and live
  in-conversation `Waiting` approval/question dialogs and `Unknown` overlays (such as model pickers) on Claude and
  Codex.

- **Drift tests: permission-mode status-line footer detection.** Verify Farhelm's live screen-footer classifier reports
  YOLO vs non-YOLO accurately on both YOLO and Guarded launches for Claude, Grok, and Muse, and YOLO on `--force`
  launches for Cursor, catching vendor footer wording or layout drift.

- **Drift tests: running-agent prompt delivery via bracketed paste.** Verify live Claude and Codex TUIs enable terminal
  bracketed paste mode, and that Farhelm's bracketed-paste plus Enter (Claude) or Tab (Codex) submits cleanly at idle,
  preserves multiline prompts without premature submission, and queues cleanly while busy.

- **Drift tests: environment and recipe credential delivery.** Using isolated Claude (`CLAUDE_CONFIG_DIR`) and Codex
  (`CODEX_HOME`) directories and a local loopback stub HTTP endpoint (zero API cost), verify real Claude and Codex CLIs
  still honor Farhelm's generated credential configuration and send requests to the configured `base_url` with the
  expected auth header.

- **Drift tests: wrapper prefixes and declared command launches.** Verify shell (`bash -c`) and `env` wrapper prefixes
  and declared command launches (`{farhelm_args}`, `{cwd}`, `{codex:trusted-cwd}`) end to end across agents,
  specifically proving conversation capture and Restart survive `exec`-based wrappers on Grok and OMP and resident
  wrappers on Claude, Codex, Muse, and Antigravity.

- **Drift tests: remote `BROWSER` opener integration.** Verify how real agent CLIs on Linux — especially Codex and
  Claude Code, which differ in whether they accept a command template or only a single executable path — invoke
  Farhelm's remote `BROWSER` opener.

## Linux support

- **Tunnels from remote sessions on Linux.** The desktop app opens Cmd/Ctrl-click tunnels (and approval cards' Tunnel
  and open) only in the Mac app; on Linux the pill, the consent switch and the card's Tunnel and open are not offered.
  Port them to the Linux desktop app: the tunnel consent switch in Settings → Terminal, Ctrl-click to tunnel a loopback
  link, the tunnel pill, and the approval card's Tunnel and open, with the same consent and refusal rules as the Mac.

- **Linux installation and updates for the desktop app.** The GPUI desktop app runs on Linux but has no install story:
  no package (tarball, .deb, AppImage or Flatpak not chosen), no desktop entry or icon install beyond what a build
  provides, and no auto-update (the updater runs only in the installed Mac app, so Linux shows no update UI). Decide the
  package form and architectures, how updates arrive (a self-updater, the package manager, or manual), and write the
  install docs and SPEC "Installation and updates"' Linux sentences to match. Known Linux issues to settle with it:
  - No desktop entry or icon is installed, so there is no menu launcher. The app id the entry must match is
    `org.scode.farhelm.desktop`, the Mac bundle id, reused on Linux until decided otherwise. A second launch brings the
    open window forward by forwarding its `XDG_ACTIVATION_TOKEN`, so the launcher must pass one for the compositor to
    allow focus.
  - A launch without a Wayland display refuses with a line on stderr only (Linux has no native alert on that path), so a
    launch from a desktop menu in an X11-only session shows nothing. X11 is not supported at all.
  - No updater and no update UI on Linux: the updater runs only in the installed Mac bundle.
  - The local supervisor is started detached and outlives the app, as on the Mac; whether an installed Linux app should
    run it some other way (a systemd user unit, say) is open. SPEC "Topology"'s Linux NOTE says how the Linux app runs
    and keeps its local supervisor is not specified yet; it needs replacing once this is decided.
  - Under KDE (KWin draws the title bar) the app is untested: no drawn bar, no GPUI Kit frame margin, the 722 by 596
    minimum, and drag, resize and close by KWin's own bar.
  - In a real GNOME session the drawn title bar's drag, double-click maximize and Minimize are untested (headless Mutter
    has no pointer drag and no visible minimize), and a close the compositor asks for (Alt+F4) reaches the app by
    another path than the drawn Close button, also untested.

## Tensorlake

### v1

#### Wave 0 — No-UX-design fixes

- **Investigate Tensorlake's ssh gateway leaving forwarded channels open, and report it if it is a bug.** Through the
  gateway, an ssh client holding a remote-forwarded connection never learned that the sandbox side was gone: the sandbox
  side was torn down within half a second, but the local ssh and its connection to the local service were still up 185
  seconds later, with keep-alives on or off, while an ordinary OpenSSH server ended the same setup in about 1.3 seconds
  and the gateway did end a connection with no forwarded channel open. Confirm what the gateway does (including whether
  it relays a forwarded channel's close in the ordinary case where only the sandbox-side client hangs up), and if it is
  a gateway bug, send Tensorlake a report with the minimal reproduction. Everything known so far, measured and inferred
  kept apart, is in `lore/2026-11-07-tensorlake-gateway-forwarded-channels.md`.

#### Wave 1 — Running, stopping, and billing safety

- **Replace plain Stop on per-session sandboxes with Stop and suspend.** Plain Stop currently stops the agent
  immediately with no warning when no extra terminal tabs are open, leaving the sandbox running and billing. Plain Stop
  on a per-session sandbox is mostly confusing; drop it in favor of only offering Stop and suspend.

- **Auto-suspend forgotten sandbox sessions.** A sandbox session bills its CPU and memory for as long as its sandbox
  runs. While the helm is connected, Tensorlake's idle timeout never fires, because the helm's connection counts as
  activity. A session the user forgot about therefore keeps billing until they suspend or delete it. The maintainer
  accepted that for the first version on 2026-11-08 and wants an automatic suspend for v1, for example after the agent
  has been idle for a while. The rule and its default are to be decided when this is picked up.

- **At-a-glance visibility into running/billing per-session sandboxes and Hosts panel polish.** Per-session sandboxes
  are omitted from the host rail, leaving no quick count or indicator in the main window of running/billing sandboxes
  without opening Hosts. Inside Hosts, replace or clean up raw `farhelm-sess-…` names and the generic
  `Settings: Edit this host's connection` menu entry.

- **Bulk Suspend, Resume, and Stop and suspend in the session list.** Support bulk Suspend, Resume, and Stop and suspend
  for sandbox sessions in the session list.

#### Wave 2 — Recipes, snapshots, and host-vs-session unification

- **Unify standalone sandbox hosts (`Add host`) and per-session sandboxes (`New sandbox`).** Reconcile the subtle
  differences between the two Tensorlake flows (auto-build vs. refuse-if-unbuilt, 24 h vs. 30 min idle timeout, rail
  icon vs. hidden, five-step creation plan vs. inline confirmation).

- **Handle unbuilt recipes in the session launcher.** Saving a recipe does not build a snapshot, and the launcher then
  disables it as `No snapshot yet` with no inline Build button or link to Recipes (whereas
  `Add host → Tensorlake sandbox` accepts an unbuilt recipe and builds and prepares it automatically).

- **Eliminate or surface the two-stage Build + hidden 1-minute Prepare stall.** Even after clicking `Build snapshot`,
  the first launch (and every 24 h or after a Farhelm update) stalls for about a minute on `Prepare snapshot`, with no
  combined Build + Prepare or explicit "Prepare now" action.

- **Align launcher agent selection with sandbox recipes.** Managed Ubuntu currently installs only Claude Code and Codex
  while the launcher presents all ten built-in agents as selectable. Design the UX for recipe-dependent harnesses —
  likely supporting installing the harness the user picks on demand.

- **Recipe maintenance friction.** Address immutable recipe names, deletion blocked by suspended sandboxes, and disk
  shrinks below the existing snapshot size failing until a manual snapshot rebuild.

#### Wave 3 — Session launcher polish

- **Remove the permanent SSH key registration command block from the session launcher.** Push SSH key registration
  exclusively into Tensorlake setup in Settings and/or the setup wizard, and make it easy to jump there from the
  launcher instead of repeating the full `tl sbx ssh keys add` block on every launch (which currently pushes the Agent
  section below the fold when Managed checkout is selected).

- **Save as template from a sandbox launch.** Restore `Save as template` in the launcher footer for sandbox launches
  (currently omitted, forcing manual rebuild in the Templates dialog).

- **Discoverability of Tensorlake sandboxes in the session launcher before setup.** The session launcher currently hides
  `New sandbox` entirely until a Tensorlake API key is saved. Surface Tensorlake as a clear option before setup is
  complete, linking into the Wave 4 setup flow.

#### Wave 4 — Holistic Settings tabs, setup wizard, and exiting Experimental

- **Holistic Tensorlake onboarding, Settings structure, and setup wizard.** Rework setup, onboarding, and Settings
  together rather than patching individual forms: consider a separate top-level tab system in Settings (with Tensorlake
  on its own tab and most existing settings under a General tab) and/or a Tensorlake setup wizard. Cover the six-stop
  first-time setup flow (Settings → Tensorlake → external `tl sbx ssh keys add` → Credentials → Recipes → Build snapshot
  → New session), starter recipe and inline credential creation in the recipe editor, fixing the crowded Settings →
  Tensorlake viewport (where the experimental banner and plaintext storage box hide the SSH key card below the fold, and
  removing/replacing the experimental warning for v1), removing the duplicated Tensorlake API key form from the top of
  Settings → Credentials, and updating the welcome card so Tensorlake sandboxes are discoverable from first launch.

#### Wave 5 — Credentials (completes v1)

- **Codex Subscription rotating refresh-token conflict across concurrent sandboxes (and broader sandbox credentials).**
  Launching multiple per-session sandboxes from a recipe using Codex Subscription copies the same `auth.json` into each
  sandbox, so the first to rotate its refresh token invalidates the others. Keep in v1 as the final wave (credentials
  are a separate track; Waves 0–4 focus on sandboxes working smoothly with an API endpoint / load balancer).

### v2

- **Agent-requested Tensorlake hosts.** Let an agent request a host by naming an existing recipe, behind the user's
  approval.

- **Live resize of Tensorlake hosts.** A host is created at its recipe's size, and a recipe's size change affects only
  hosts created afterwards. Tensorlake can resize a running sandbox in place (CAS image format only): it takes one to
  two seconds and keeps processes running; CPUs go up and down, but memory never shrinks below what the sandbox booted
  with, so a sandbox meant to flex has to boot small. Let the user resize a Tensorlake host (or apply its recipe's
  current size) without recreating it. Phase 1 left this out deliberately.

- **Live resize for sandbox sessions.** Let a session's sandbox boot small and grow on demand, preserving its running
  processes. Build on "Live resize of Tensorlake hosts" above; expose it from the session's controls. The first version
  launches at the recipe's size and offers no resize control.

- **Per-repository warm snapshots for sandbox sessions.** Build on a recipe's prepared snapshot, then refresh each
  checkout with an incremental `git fetch` when a session starts. Decide how these snapshots are refreshed. Keep using
  the recipe's GitHub token delivery to each sandbox for private repositories. Design records:
  `lore/2026-11-03-tensorlake-sandbox-hosts-design.md` ("Phase 2, as far as it was thought through", plus the measured
  platform facts) and `lore/2026-09-29-per-session-sandboxes-as-the-remote-host-goal.md`.

- **Possibly: shared scratch volumes for sandbox sessions.** Explore Tensorlake cloud volumes as scratch space shared by
  sandbox sessions.

- **Possibly: fork a warm sandbox session.** Explore a memory checkpoint or live copy that preserves running processes.
  A copied supervisor retains its identity and the sandbox's boot ID, so an independent fork needs an explicit answer to
  that ownership problem. Ordinary Clone starts a fresh sandbox from the recipe instead.

## Mid term

- **Select-all key for Linux text fields.** Since the emacs-text-field-keys plan, Ctrl+A in a Linux text field moves to
  the start of the line, as on macOS, so Linux text fields have no select-all key (no Edit menu, and fields have no
  context menu). Decide how and whether to have one.

- **Built-in questions tool.** A built-in questions tool in the style of the interactive artifacts: how an agent asks
  the user a structured question (multiple choice, say), presented like those artifacts rather than as plain text.

- Initial version of agent2agent comms.

- **Remove the startup shutdown of the old private tmux server ("phase 3").** The tmux removal plans
  (`tmux-end-old-terminals` and its followers) keep one piece of tmux on purpose: a supervisor that finds the private
  tmux server an older release left running turns it off at startup, marks the sessions that ran in it as interrupted
  and tells the user. Once no installation can still hold such a server, remove that step and what exists only for it:
  the startup code and its tests, the stored terminal-kind column and its reads (a schema change), the old-terminal
  notification kinds, the scrub of `FARHELM_TMUX` from session environments, and the test fixtures that build a server
  in the old layout. Asked for by the maintainer on 2026-11-11 while planning the removal. When that is safe (a release
  count, or the upgrade floor passing the release that ended old terminals) is to be decided when this is picked up.

- **Re-assess letting agents prompt running sessions.** `farhelm agent prompt` (an agent sending a message into another
  session's running Claude or Codex agent) landed and was turned off before any release: the helm refuses every such
  request, and the command is hidden from agents. One gap remains where a delivered message could do something the user
  never agreed to. The "is it waiting on a question?" check uses a status reading up to about two seconds old, so a
  permission or trust question that appears in that window, or right after the agent starts, can receive the message and
  its Enter as its answer. Re-enabling means fixing that gap or deciding to accept it, then setting
  `AGENT_PROMPTS_ENABLED` in `crates/farhelm-helm/src/agent_requests.rs` to true, unhiding the CLI verb, restoring its
  agent instructions and docs, un-ignoring the end-to-end delivery tests, and writing a new changelog fragment. SPEC.md,
  Agent-spawned sessions, keeps the design.

- **Support Kimi Code.** Add Moonshot's Kimi Code CLI as an agent type: terminal launches with guarded and YOLO
  permission flags, model, effort and prompt choices once their command-line contracts are verified, a harness mark and
  docs page, then session tracking — per-launch hooks/instructions, conversation capture for resume, and waiting/status
  recognition.

- **Support DeepSeek's coding agent.** DeepSeek publishes models, but no clearly official terminal coding agent was
  found on 2026-10-28; the DeepSeek CLIs that turn up are third-party. Confirm the product and its binary first, then
  add it the way Kimi Code is to be added.

- **Consider auto-scaling the transfer window.** Uploads, downloads and tunnel relays between the helm and a supervisor
  each have a fixed 4 MiB credit window, so one stream's throughput is capped near 4 MiB per round trip whatever the
  link can carry. Asked for by the maintainer on 2026-11-08 while planning the chunk-size change
  (`plans/queue/mux-chunk-size.md`): consider sizing the window automatically from bandwidth and round-trip time,
  bearing in mind that on remote hosts ssh's own channel window, shared by every stream, may be the real cap.

- **Cap the Mac app's own localhost tunnels.** Recipe tunnels are capped at 64 tunneled connections per host, but the
  relays the helm opens for the desktop app's localhost tunnels take no permit, so their number is unbounded
  (`crates/farhelm-supervisor/src/service/tcp_relays.rs`, around the relay start). Asked for by the maintainer on
  2026-11-08 while planning the chunk-size change.

- **Search sessions by content or action.** Let the user find a session by what happened in it — which session ran a
  certain task, command, or produced a given output — rather than only by name. What gets indexed and how the search is
  exposed are TBD.

- **Make auto-tunneling safe.** Figure out how to make auto-tunneling properly safe: can we use some hostname trick so
  the service does not present as localhost? TBD. Pinggy? TBD.

- **Restore automatic recovery from a withdrawn waiting update.** On 2026-10-14 the signed release advisories plan was
  narrowed to warning about a withdrawn running version and refusing to install a withdrawn release; a withdrawn release
  that is already installed and waiting for a restart now needs manual recovery, and an ordinary relaunch may still open
  it. The plan, with the blocked question and the decision, is
  https://github.com/scode/farhelm/blob/d8f02862ce4014255d02d4383910999b61c85dbb/plans/queue/release-advisories.md.
  Automatic replacement was blocked by what execution found about supervisor startup: a supervisor claims its state and
  migrates its session database before it checks tmux and before it records its running version. If startup fails or
  stops in that window, the database is newer than the record says (or there is no record), so an update check that
  trusts the record could install an older release that then refuses the database. The ordering exists in shipped
  releases (v0.28.2 included) and was reproduced with a migration from schema 30 to 31 followed by a tmux refusal while
  the record still named 0.23.0. The follow-up: have a supervisor durably record the version it is about to run before
  it can migrate any state, handle compatibility conservatively when it cannot be established (existing installations
  and interrupted startups included, without treating old records as trustworthy retroactively), check how commands
  inside sessions select a binary under the new meaning of the record, and then restore the automatic replacement. Start
  with a focused review of supervisor startup and interruption before implementing.

- **Release stable from the approved RC without rebuilding.** When an RC has been built and declared correct, releasing
  stable should default to promoting that RC's artifacts, not rebuilding everything for the stable tag. Today a stable
  is cut from main and the tag-triggered workflow builds every target afresh, so stable binaries differ from the
  approved RC's even when nothing changed. First find out what prevents reusing the RC builds: whether cargo-dist (the
  generated `release.yml`, driven by `dist-workspace.toml`) can publish the RC's artifacts under the stable version, and
  what the release commit, version settlement, curated notes and the maintainer's signing of the six payloads demand of
  the build.

- **Scroll the grid terminal entirely from a back buffer it keeps itself, instead of history sent by the supervisor.**
  The page already keeps the scrollback it shows and scrolling sends nothing
  (`crates/farhelm-ui/assets/grid-terminal.js`, "Scrollback"), but that history arrives as `history` messages from the
  supervisor's emulator, one message per line that scrolled off, each line at the width it was written
  (`crates/farhelm-proto/src/grid.rs`, "History"), and a `history_reset` rebuilds it whenever the emulator resizes.
  Consider dropping that half of the stream: the grid client appends the rows a `frame.shift` pushed off the top to a
  back buffer of its own, keeping each line at the size it was shown at (the terminal's current size, not reflowed by a
  later resize), so scrolling never depends on the supervisor re-reading its emulator. Two constraints decide whether
  that works. Flow control skips screen frames while a client is behind but still sends every line that scrolled off as
  history, which is how it never loses output (SPEC.md), so a buffer fed only by the frames the client saw would have
  gaps; and a client attaching to a terminal that already has output has no local history to show. Both need an answer
  before the shape is settled. Related to, but not the same as, the grid stream's binary encoding (grid-binary-stream,
  landed 2026-11-09), which kept the split and changed how the same lines are encoded.

- **Look into the GPUI frame-source patch further.** The new desktop app carries a patch to its vendored GPUI that keeps
  GPUI's older macOS frame source (`CVDisplayLink`) on macOS 14 and later, instead of the `CAMetalDisplayLink` GPUI uses
  there by default. Without it, a key press took about 55 ms to reach the screen on the test Mac VM, against about 35 ms
  in the old app and about 30 ms with the patch, because the newer source presents a frame about three refreshes after
  it was drawn. The patch costs idle CPU (about 0.4% instead of 0.1%) and gives up macOS choosing the frame rate
  (ProMotion displays, low-power mode). Kept on 2026-11-13; look into whether Apple or GPUI may eventually drop the
  older path, whether the newer source can be made as fast (setting its frame latency to 1 made no difference on the
  VM), and how it behaves on real hardware with a ProMotion display.

- **Revisit drawing inactive windows at full frame rate.** The new desktop app draws a window at full rate even when
  another app is in front, matching the old app, instead of GPUI's default cap of 30 frames a second. That keeps output
  scrolling smoothly in a window you are watching but not using, at the cost of more drawing (and battery) while output
  scrolls in a background window, which is common while agents work. Decided on 2026-11-13 to match today; revisit the
  tradeoff.

- **Revisit the KDE-blur dependency decision.** GPUI's Linux backend pulls in `wayland-protocols-plasma`, a crate
  declared MIT that bundles KDE Wayland protocol XML files licensed LGPL-2.1-or-later, which are compiled into generated
  bindings in the Linux desktop app. GPUI uses it only as a fallback for blurring a window's background on older KDE
  Plasma, and Farhelm never asks for a blurred window. Decided on 2026-11-13: remove it with a small patch to the
  vendored GPUI if that needs no horrible hacks, otherwise accept it as protocol definitions rather than code. Revisit
  whichever was done, and whether the license check should catch GPL-family files bundled inside a crate whose declared
  license is permissive.

- **Command and control drop-down on Command-backtick.** command and control bound to `` command-` `` (drops down)

- **Agent-operable config through command and control.** agent operable config through command and control

- **A UI activity log that feedback can include.** Keep a timestamped log of what the UI did (dialogs, focus, shortcuts,
  requests to the helm and how long they took), and add an "Include application logs" checkbox to Send feedback. The
  point is general: whenever "I did X and something odd happened" or "I did X and it took about 5 seconds" comes up, the
  maintainer files feedback right after noticing, and the log says what actually happened and where the time went,
  instead of an agent guessing from code. The motivating example: a launch typed as `gh:owner/repo` sometimes ended up
  as a session named exactly that in the wrong repository, and code reading could only speculate that the "Rename
  session" prompt after a quick launch took the typing. A log reading "shortcut ⌘P → quick launch; dialog opened: Rename
  session (after quick launch); focus → rename field; typed 11 characters; Enter → submitted" would have settled it in
  one report.

  What to log: semantic events, not raw input. Which shortcut matched and which action it dispatched; dialogs opening
  and closing, with their kind and why they opened where that differs (the post-quick-launch naming prompt versus an
  ordinary Rename); focus moving between named controls; submit, cancel and Escape; launches and other operations
  starting and completing; and every request to the helm with its start, end, outcome and duration. Timing is a
  first-class requirement, not a nicety: every event carries a monotonic timestamp with millisecond resolution (not only
  wall clock), and anything with a duration logs both ends, so "it took like 5 seconds" can be broken down into which
  request, wait or render was slow. Ideally the helm logs the same requests with its own timing under a request id the
  UI log also records, so a slow step can be split into UI, network, helm and host time; how far that correlation goes
  is a design question.

  Privacy, by construction rather than by care: never log typed characters, only counts ("typed 11 characters into
  Name"), since fields hold prompts, paths, repository and host names, and secrets such as API keys. Model events as a
  closed set of typed kinds rather than free-form strings, so a later edit cannot leak a session title or path by
  accident, and name controls by Farhelm's own labels, never by page content or peer-supplied values. Requests are
  logged by route shape, not by the ids, paths or names inside them. The checkbox reveals the exact log text that will
  be sent before Send, keeping the dialog's existing contract that what it shows is the whole submission. A SPEC.md
  "Feedback" update has to state this promise.

  Shape: an in-memory ring buffer per window (last few hundred events or about 10 to 15 minutes, to be tuned), never
  written to disk and never sent anywhere until the box is ticked. This covers the "only noticed afterwards" case as
  long as feedback is filed soon after. Most events can come from a few shared places rather than per-call-site logging:
  the shared modal dialog component, the shortcut dispatcher, one page-wide focus listener, and the API client for
  request timing.

  Caveats:
  - Size. Feedback is capped today at a 4000-character message (shared by the UI, the helm and the farhelm.io endpoint)
    and a 64 KiB request body (the helm and the endpoint); the maintainer plans to raise those limits. Compress the log
    in the client before sending. The dialog still shows the uncompressed text.
  - Where the log lands. The endpoint files each submission as an issue in a private GitHub repository, and an issue
    body is limited to roughly 65,000 characters and is not a place for a compressed blob. The endpoint will need to
    decompress and split the log across comments, or store it elsewhere and link it; decide before building the client
    half.
  - Who can read it. Whoever can see that private repository can read every included log, which is one more reason the
    privacy rules above must hold without exception.
  - The helm side has no file logging yet (stderr only, per SPEC_impl.md), so helm-side timing for a report would have
    to come from an in-memory buffer as well, or wait for file logging.

## Doc todo

- Tell users that a working-copy (checkout) root must be a location only they can write: Farhelm is not designed for
  roots shared with or writable by other local accounts, and does not check. SPEC.md's fresh-checkout section states the
  rule.
- Tell users that closing, exiting, or deleting a terminal tab also stops services the tab shell's startup files started
  there first (an `ssh-agent`, an editor daemon, a detached tmux server), and that services the agent's launch started
  first are not affected. A tmux autostart guarded by `TMUX` fires during the agent's launch, and one that execs tmux
  keeps the agent from starting. Likewise, a personal tmux server or an ssh ControlMaster that the agent is the first to
  start ends with Stop, Restart, Replace or Delete, and one a command in a tab is the first to start ends when the tab
  closes or the session is deleted, each with everything the user later ran in it or through it, including from outside
  Farhelm. SPEC.md's session view section and "Ownership during cleanup and provisioning" state the rules.
- Tell users what Stop, Restart, Delete and closing a tab can and cannot kill on a Mac, and on a Linux host whose
  supervisor runs without a usable systemd user manager. There, a background process that has detached from the
  session's terminal is found only through an environment marker, so one that hides or overwrites its environment
  survives: non-dumpable and setuid programs, and daemons that rewrite their process title, such as nginx with default
  settings or Postgres started through `pg_ctl`. Servers kept in the foreground under the agent are still stopped. Point
  Mac users at running such servers in the foreground, or stopping them themselves. SPEC.md's lifecycle operations
  section states the rule.
- Tell users that bash and zsh are the supported login shells on every host: agents and terminal tabs start through the
  login shell, and other shells, csh and tcsh included, may fail to launch them. SPEC.md's supported user environments
  section states the rule.

## Tricky bugs

## Deflake

- **Wait for the terminal-host helper to restore stdout in the descendant PTY test.**
  `the_exit_is_reported_while_a_descendant_holds_the_pty` in `crates/terminal-host/tests/host_lifecycle.rs` failed in
  the v0.44.1 Linux release gate when it sampled stdout during the helper's readiness FIFO write. Failure evidence
  `0363fbf7-7a8f-4f6d-b2cb-b627e371d917`. The isolated retry `f1492329-18dc-4d14-a6c6-6610282d20f5` and hosted retry job
  `137328954707` passed the test. The fixture should make the descriptor restoration observable before asserting that
  the descendant still owns a PTY.

### Difficult deflake

## Broken tests

## Code cleanup

### Turning rules into structure

A 2026-10-19 assessment of about 530 recent product fixes found that most were an agent missing a rule, written as prose
or per-call-site discipline, at one place when a sibling path already followed it. The five refactors chosen from it
landed on 2026-10-21. A second assessment that day found that recurring fixes had since moved mostly to UI state and
interaction. After an adversarial review, the maintainer chose the five behavior-preserving refactors below, each of
which makes one such rule something a type, a single owner or a default enforces. A behavior change aligned with the
problem a refactor targets (making drifted copies consistent, say) is allowed in its own `fix:` PR; unrelated changes
are not.

## Maybe later

- **Reconsider terminal-native as a style for the app.** Direction 8 in
  [the dialog directions mockup](https://snippets.scode.org/s/farhelm-dialog-directions/) (everything in the terminal
  font, a prompt and block caret, fzf-style counts, dashed rules) was well liked, but the launcher's look won for the
  Cmd+K, Cmd+J and Cmd+L dialogs because it is consistent with what the launcher and Settings already have. Reconsider
  it as a general style.

- **Park the docs-shot pointer on a quiet point when no element qualifies.** Before each docs screenshot, the pointer is
  parked on an element with an id that nothing reacts to. A redesign that removes the last such element (as the
  launcher's title row went) breaks every shot of that surface until each gets its own park target. Fall back to a point
  in a dialog's padding, a gap between controls or the window margin, clear of every hover and tooltip area and of
  popovers. That needs the vendored GUI bridge (`vendor/gpui-agent`, changed together with its patch file) to hover at a
  coordinate, and on a Mac, where the real pointer moves, the point park has to work too.

- **Restore deleted sessions.** Support undeleting a session that is in the trash. Consider supporting undelete for
  sessions without a managed checkout too, by retaining their metadata.

- **Protect the repository against force pushes and similar, while agents can still merge.** Guard main and the release
  tags against force pushes, deletion and other history rewrites in a way that also binds agents, while still letting
  them open, publish and merge PRs as they do today. Main's branch protection already disallows force pushes and
  deletion, but it does not apply to administrators, and agents work through the maintainer's own GitHub account, so it
  does not stop them. Tags have no protection at all (the repository has no rulesets), though the release procedure says
  a tag is never deleted or reused. Work out what GitHub rulesets, bypass lists or a separate agent identity can enforce
  without blocking the PR flow.

- **Reduce the peak memory of snapshots made mostly of cell fixups.** The terminal snapshot encoder still builds cell
  fixups in their own buffer and copies them after the stream, so a fixup-heavy snapshot peaks near twice its size.
  Painting those bytes in place too needs a separate design; the ordinary stream-heavy case already reuses its final
  blob's allocation.

- **Crappy model warning.** Let the user pick a list of models they never want to run. Warn when an agent's TUI
  indicates that one of those models is being used anyway.

- **Input-method typing in grid terminals.** Terminals that run in a terminal host are drawn by the grid terminal
  client, which does not show an input method's composition at the cursor or place the candidate window there: composed
  text reaches the program once committed, but the composing itself happens out of sight. Doing it the way xterm.js does
  means porting its composition helper (a preview element and an input element kept at the cursor on every render, plus
  Chrome and Korean input that arrives outside composition events), which failed the complexity gate of the
  terminal-host work. Verifying it needs real input methods, not only synthetic composition events.

- **Let an agent delete its own session.** `farhelm agent delete` refuses a session's own id (plan
  `agent-delete-and-prompt`), because the request holds a lock keyed by the asking session that the delete would wait
  on, and the session's credential is revoked mid-request; SPEC.md declines `farhelm agent replace` for the same reason.
  Allowing it needs a shape where the helm answers first and deletes afterwards. Decided 2026-10-16 while planning agent
  deletion.

- **Reconsider an ordered WebSocket channel between the GUI and the helm.** On 2026-10-15 the maintainer chose ordering
  in the GUI instead (a request waits only for this GUI's unanswered saves; plan `ordered-gui-helm-requests`), after a
  planning deep dive. The deep dive found that the helm could replay WebSocket frames into its existing routes
  unchanged, but estimated about 5,000 to 8,000 lines over 7 to 10 PRs. The largest single cost is migrating the browser
  tests, which intercept API calls over HTTP in 461 places that would not see WebSocket frames. It also leaves lasting
  complexity: plain HTTP must stay for tabs left open across a helm upgrade, every new route needs an ordered-or-not
  decision, middleware must be kept in step across two ways into the helm, requests disappear from browser devtools as
  separate entries, and one stalled socket stalls every request. Its one gain over GUI-side ordering is fewer
  connections over a high-latency tunnel. Large transfers (attachments, downloads), terminals and the event feed would
  stay off the channel. Revisit if remote browser use over slow links becomes common, or if GUI-side ordering proves
  insufficient.

- **Further harden the release and update trust chain.** The chain is already hardened: installs and updates come from
  get.farhelm.io with signed checksums, and CI no longer signs. Further steps would raise the bar: a Vercel user that
  holds only the get.farhelm.io project; disconnect the docs project from Git (its automatic deploys are off only by a
  setting in agent-writable `website/vercel.json`); tag rulesets and GitHub immutable releases; lock down the DNS,
  registrar and email accounts; and review the source diff before signing, since the signature proves who published a
  release, not that its code is sound.

- **Experiment: a different allocator for the release binaries.** EXPERIMENT ONLY; nothing changes without a measured
  win. The release binaries are static musl, whose allocator hands memory back to the OS eagerly; about a quarter of the
  supervisor's CPU was the kernel's cross-CPU interrupts from those `munmap` calls, though most of the churn came from
  polling that has since been removed (#1738, #1766). Profile the supervisor again, try mimalloc or jemalloc, and keep
  it only if a before/after measurement on the same workload shows a clear CPU win worth building a C allocator for
  every release target. Baseline numbers and method: `lore/2026-10-08-supervisor-idle-cpu.md`.

- **Install Claude and Codex on a host for the user.** Offer to install Claude Code and Codex (only those two) on a host
  on the user's behalf, always through each vendor's one-line curl installer, and optionally copy the user's credentials
  for them over to the host. Choosing to copy credentials must come with a warning that it is the user's call, including
  the risk that a provider bans the account for use across hosts. Offer it in two places: as an action in the host's
  pop-up menu, and as an opt-in option (off by default) when adding a host. Relatedly, adding a host should probe it
  once Farhelm's own installation there is complete.

- **Guard the website's install command against a rogue change.** Add automated protection so that an agent cannot
  replace the `curl … | sh` install command on the docs website without the maintainer noticing. The command is what new
  users paste into a shell unread, and it appears in three places (the site header, the Install page, and the README)
  that nothing checks today. Agents land changes to main without the maintainer reviewing each one (the plans monitor
  does, for one) and can trigger a website deploy, so a changed command could reach the live site unseen. What the
  protection checks, and where (the website build, a check against the live site, an alert to the maintainer), is to be
  decided when the entry is planned.

- **Brick test a release before signing it.** Today the brick test can only run after the maintainer has signed the
  release and published it at its version path on get.farhelm.io (it does not need `/latest` moved). The signature is
  about security, not about the release being free of bugs, so needing it first is not a bug in itself. But it ties the
  two steps together, and ideally the brick test would not need a signature, so the maintainer could brick test a
  release separately from signing it. Several parts of the test lean on the signature. The installer and the helm only
  fetch releases from get.farhelm.io, which only serves signed ones. The helm checks the signature on the remote-host
  payloads it downloads. The old app's in-app updater checks it against the keys compiled into that old release. And the
  brick test's first check verifies the candidate against the previous release's keys. The trade-off is realism: every
  way around the signature (a mirror, a test-only key, skipping verification) makes the brick test exercise something
  other than what a user's upgrade actually does.

- **Require a systemd user manager on Linux, with no fallback.** Decided 2026-10-02: on Linux, a usable `systemd --user`
  is simply required, and Farhelm has no fallback for working without one; macOS has no systemd and keeps the portable
  process sweep. A Linux host whose user manager is missing, hung or broken is a broken host, like one with an I/O error
  or a full disk, not a mode Farhelm adapts to. What is left is the implementation, not the decision.

  Today there are two fallbacks. A Linux supervisor that finds no usable user manager uses the same sweep macOS uses,
  with the weaker cleanup guarantee SPEC.md's lifecycle operations section describes; a definite "no usable manager"
  answer is cached for the supervisor's lifetime, while a probe that times out is retried a minute later. And since
  #1279, a launch that meets a slow user manager still starts, just without a scope, so that session relies on the sweep
  for good while later launches check again. Both go. A launch or tab open on Linux that cannot get its scope fails
  visibly instead of starting unscoped, and a supervisor that once found the manager unusable checks again rather than
  keeping that verdict. The code saving is small, since the sweep itself is shared with macOS and only the `/proc`
  reader is Linux-specific. The real gain is that a Linux host stops flip-flopping between systemd and no systemd: scope
  use is decided per launch and per tab against a cached verdict, so one session can mix scoped and unscoped processes,
  and a stale "no usable manager" verdict can make cleanup skip a scope that does exist. That mixing breeds edge cases
  like these two, found in review-feedback triage on 2026-10-02 and folded into this entry rather than fixed separately:
  - Delete skips a tab's scope (`delete-skips-scoped-tab-on-stale-verdict.md`). An agent launched without a scope
    because systemd was briefly unreachable, a terminal tab later opened inside one, and a supervisor whose first
    systemd check after a restart failed: Delete then never checks the tab's scope and reports success, and a process
    the sweep cannot see (such as an `ssh-agent` started by the tab's shell startup files) outlives the session with
    nothing left that can find it.
  - A failed create orphans its scope (`create-rollback-orphans-unconfirmed-scope.md`). When a create fails after the
    agent started and systemd does not confirm the scope's kill, the rollback still removes the half-created session if
    the sweep came up clean, so a daemon the sweep cannot see keeps running with no session to Delete. Dropping the
    fallback does not change this by itself, because the trigger is a hung manager at rollback time. Make the rollback
    behave like every other cleanup Farhelm cannot confirm: the create fails visibly and the session is kept, so a later
    Delete can retry once the host is healthy.

  A related case was folded into this entry on 2026-10-11 for the shared rollback site and remedy, rather than scoped
  and unscoped process mixing:
  - A failed database confirmation discards cleanup ownership
    (`failed-confirmation-discard-session-despite-failed-process.md`). When a create's database confirmation fails and
    its process sweep also fails, a confirmed end of the session's terminal host still drops the session record. A
    surviving detached process then keeps running with no session left to Delete. The trigger is a local storage
    failure. Keep the session when cleanup cannot be confirmed, so a later Delete can retry.

  When executing, check that all three cases are actually gone.

  Once the fallback is gone, also assess what the process sweep still has to do on Linux (to look into, not decided,
  2026-10-05). With every launch and tab in a scope, the walk from the process numbers of a session's terminals (their
  hosts' children) has no Linux reason left, since its stated purpose is hosts without tab scopes, and dropping it there
  removes the short process-number reuse race SPEC_impl.md currently accepts as an interim measure. Whether the
  environment-marker sweep should stay as a backstop, for processes that left the scope but kept the session marker (an
  agent running `systemd-run --user --scope`, say), is part of the assessment.

- **Decide what process cleanup promises on macOS.** macOS offers Farhelm no equivalent of a cgroup scope. launchd
  tracks a job by its process group, which `setsid` escapes, and the kernel's coalitions, the closest analogue, are as
  far as we know private API (not verified). So Stop and Delete there rely on the walk down from the terminal's process
  and the environment marker, with the residuals SPEC.md's lifecycle section lists. Work out what Farhelm should promise
  and do on macOS, and how much of that machinery earns its keep: for example whether the walk from the terminals'
  process numbers is worth it, and whether the short process-number reuse races SPEC_impl.md accepted on 2026-10-05 as
  an interim measure should stay accepted for good.

- **Goose foreground ownership.** Keep the basic Goose reporter for now; it does not distinguish a native or shelled-out
  child that inherits the reporter from the foreground conversation. The stricter database-backed implementation is
  preserved at the `goose-capture-complex-2026-09-23` tag for comparison. The research and proposed smaller replacement
  are recorded in [the Goose session-tracking assessment](lore/2026-09-23-goose-session-tracking.md). Revisit only if
  reliable child isolation becomes a product requirement or Goose exposes a direct root/subagent role signal.

- **Pi foreground ownership.** Assess whether native or shelled-out Pi children can replace or withdraw the foreground
  conversation's restart target, then define the smallest admission check that preserves legitimate foreground
  transitions. This is an assessment task, not a claim that every vendor path has been reproduced.

- **OMP foreground ownership.** Assess whether native or shelled-out OMP children can replace or withdraw the foreground
  conversation's restart target beyond the checks now landed in #814, and define any remaining smallest admission check.
  Preserve legitimate foreground transitions and avoid extending the reporter's scope without evidence. The earlier
  cross-harness evidence for this and the Pi entry is preserved in
  [the historical ownership assessment](lore/2026-09-20-harness-conversation-ownership.md).

- Reconsider the first-use configuration experience for `gh:` launches when no working-copy root is configured. The
  first version refuses the launch and points to the CLI command; consider an inline GUI flow on initial use or another
  improvement that makes setup easier. Keep the general preference for CLI configuration of rarely changed settings.

- Reconsider agent parent/child relationships: either remove them or make them useful. Current parent tracking is
  optional, and fleet `agent create`/`clone` do not record the asking session, so the parent filter cannot reliably
  answer which sessions an agent created. This is largely unused complexity today; assess whether useful tracking is
  worth keeping before extending it. The current limitation is explicitly accepted in SPEC.md.

- Close the read side of what attached hosts can learn about the fleet. Acting across hosts already needs the user's
  approval (or the requesting host's "run farhelm commands without asking" setting), but any attached host can still
  read the fleet-wide session and host listings and, through an approved spawn that applies a template, any template's
  full contents including command lines, and through an approved clone any session's command line; SPEC.md's Local
  authority section accepts that only until this lands. The end state: only explicitly trusted environments may
  interrogate other hosts' session and template data. Preserve the eventual ability for agents to orchestrate sessions
  across hosts through an explicitly authorized launch policy, potentially trusted templates, without a card for every
  step.

- Make agent hook installation an explicit step surfaced to the user: tell them which hooks Farhelm installs for which
  agents, and have them accept specific hooks, so Codex launches no longer need `--dangerously-bypass-hook-trust` and
  its `-c` overrides. Today that bypass lets unreviewed hooks in the user's Codex config home and in a trusted
  workspace's `.codex/` run; SPEC_impl.md accepts it only until this step exists.

- Use the per-host yolo safe / sensitive setting (from the Near term entry "Guard against yolo launches on sensitive
  hosts") to decide which hosts appear red in the session list. The launch warnings this entry originally also proposed
  moved into that Near term entry.

- Support agents' native voice modes by piping audio through the helm to the supervisor and onward to the agent. Assess
  feasibility later, including how agents accept audio and what transport or audio-device integration would be needed;
  this entry does not commit to a design.

- Take a pre-upgrade backup of the on-disk state so a release can be rolled back. Rerunning the installer pinned to an
  older `FARHELM_VERSION` swaps the binaries back cleanly, but it does not make a downgrade work: both stores refuse to
  open a database whose `user_version` is above what the binary understands (deliberately — misreading is worse than
  refusing), so any release that bumps a schema (0.3.0 takes both the helm and the supervisor from 14 to 15) leaves the
  older binary unable to open the state it finds. Today the only rollback is a state-directory backup taken by hand
  before upgrading (the 2026-08-31 upgrade kept one next to the old binary), and remote hosts updated by the helm have
  the same problem for their own supervisor state with nobody taking a backup at all. Wanted: the upgrade paths — the
  installer for the local machine, and the helm's host update for remote ones — snapshot the state directory (a copy, or
  SQLite's backup API, taken while the old version is stopped) before the new version first opens it, keep a bounded
  number of such snapshots, and document the restore. Noted 2026-09-03 while cutting 0.3.0-rc.1.

- Consider dropping the race-proofing around host identity, keeping the identity itself. To be clear about what stays:
  the per-install identity the supervisor mints on first run and stores in its own database, independent of hostname and
  address, so a retargeted row or a state directory moved to another machine is recognized as the same install; "never
  silently merge" as a user-visible rule; and the mismatch surfaced with both identities and an adopt choice. What goes
  is the machinery that closes millisecond windows in RECORDING it: the empty-slot-only compare-and-swap in
  `record_first_contact`, the dialed-configuration check inside the same transaction (a retarget straddling a
  handshake), the separate adopt CAS with in-transaction cache purge, the never-reused connection tokens that every
  session-cache and mutation write carries, and the split of one "something is off" situation into three connection
  states (`identity-mismatch`, `duplicate`, `identity-unverified`) each with its own remedy text and re-probe policy —
  about 1,500 lines of helm implementation plus ~2,000 of tests across `store.rs`, `manager.rs`, and `hosts.rs`. The
  replacement is check-and-ask: on connect, compare the reported identity with the stored one; equal or empty means
  record and proceed, different means freeze and ask. The races it stops defending against need a retarget or a second
  first-contact to land within one handshake of another, and even then the consequence is a wrong identity the next
  connect flags as a mismatch, not a merge nobody sees. SPEC_impl.md's "structurally impossible at the storage layer"
  and "SCHEMA invariant" paragraphs under Helm internals would be rewritten to say the check is a check. Write-up:
  https://claude.ai/code/artifact/c3d3a74b-ae55-45c7-b3ca-fe30f9f97432

- Replace `install.sh`'s park/journal/rollback with plain idempotency. The floor to keep: re-running the installer from
  ANY intermediate state converges to a correct install, and no single binary is ever torn — download and extract into a
  staging directory inside `$INSTALL_DIR`, verify, then `mv` each binary into place, which is an atomic rename on one
  filesystem. Keep the outer brace group too; it is two lines and is what makes a truncated `curl | sh` execute nothing.
  What goes is the transactional layer built on top of that: the `mkdir` lock with ownership checks, the
  `PARK`/`INSTALL`/`UNDONE` journal, `rollback_from_journal`, the two rollback branches in the replacement loop, the
  `.old` parking files and the refuse-unless helpers around them — about 220 of the script's 527 logic lines (the other
  ~570 lines of the file are comments). Be honest about the size: the ~300 logic lines that remain are things any
  correct installer needs — target detection with the Rosetta case, prerequisite probing, latest-version discovery,
  `SHA256SUMS` handling, tar member validation, the `--version` cross-check, PATH advice, the closing messages — and
  `test-install-sh.sh` mostly tests THOSE (404, checksum mismatch, malformed archives, versions, prerequisites, the
  closing-message contract, the nothing-outside-`$INSTALL_DIR` diff); only the forced-failure rollback leg goes. Given
  up: a kill between the two macOS binaries' renames leaves one new and one old until the next run (the desktop shell
  finds its sibling by path, so a mismatch shows as a refusal, not silence), and a failure after placement no longer
  restores the previous binaries — re-run instead. A modest cut, worth taking when someone is in the file anyway rather
  than on its own.

- Fly.io Sprites as a host kind: a session backed by a per-second-billed microVM that freezes when idle, with "pause
  this host" in the UI meaning "stop paying for it". Assessed 2026-08-30 against a real sprite; the findings, the code
  mapping (a `HostKind::Sprite` over the existing ssh transport via the sprite CLI's ProxyCommand emulation, a
  provisioning flavor for a host with no systemd and no sftp, a `Paused` host state), the SPEC conflicts to surface, and
  a build order are in `lore/2026-08-30-fly-sprites-as-a-host-kind.md`. The same entry sizes the related "native app
  attaches to a remote helm" mode and the cheaper installed-web-app alternative.

- **Pluggable host creation.** Let users plug in their own scripts that spawn and destroy hosts for sessions to run on,
  rather than Farhelm knowing each provider itself. Related to Tensorlake hosts and the Sprites host-kind entry above,
  which build specific providers in.

- Sandboxed agents with scoped GitLab credentials, especially on Tensorlake hosts and the proposed Fly.io Sprites hosts:
  give an agent a token that reaches only the repositories it works on, minted per sandbox. GitLab rather than GitHub
  because GitLab lets the token creation be automated, while GitHub's scoped tokens have to be created by hand in a
  browser.

- Extend container sessions to macOS container hosts, help users prepare or obtain images, and consider stronger
  isolation beyond the experimental Linux rootless Podman support.

- **"Learn more" links from the GUI to the docs.** Once the official documentation lives at a stable URL, add direct
  "Learn more…" style links in various places in the GUI that point at the relevant documentation page. Which places get
  one is to be decided when this is picked up.

- **In-app intro guide.** Build an introductory guide into the app itself, so a new user can learn Farhelm's core
  concepts and first steps without leaving it. Form and content are to be decided when this is picked up.

- **Consider allowing exactly one UI attached to the helm.** A single GUI attached to the helm is the supported user
  surface, and several concurrent GUIs (browser tabs, the desktop app, other devices) are best effort. Consider going
  further and refusing a second concurrent UI outright, for simplicity: races between clients would stop being possible
  rather than merely unsupported. Part of deciding is what replaces today's multi-client machinery that SPEC.md's
  session view section specifies (one attached client per session, takeover with a displaced snapshot and take-control
  action), including how moving between devices would work, and that agents acting through fleet operations are not a UI
  and would still act concurrently. Came up in review-feedback triage on 2026-10-01.

- **Inject commands into agents from the UI.** A button such as "stop all work and restart" could send a message to a
  running agent. `farhelm agent prompt` holds the delivery path for Claude and Codex but is turned off for now (see
  "Re-assess letting agents prompt running sessions", Mid term); a UI button and delivery to other agents remain
  separate work. See SPEC.md, Agent-spawned sessions.

- **Make copying work out of the box with Codex.** Since Codex 0.157 its fullscreen interface captures the mouse, so a
  plain drag in its prompt box highlights text without copying it; the drag-copy hint plan only explains this to the
  user. Consider a launcher option to turn Codex's fullscreen mode off (`-c tui.fullscreen_transcript=false`, the
  setting `/tui` → Scrollback saves), or another way to make copying work without the user having to know Codex's copy
  key. Decided 2026-10-02 while planning the hint.

- **Whole-app zoom.** The font size plan covers terminal text only. Consider a zoom for the whole UI as well: the
  desktop app's webview has a native zoom Farhelm never uses (Cmd/Ctrl +/− do nothing there today), while on the web the
  browser's own page zoom already works but cannot be driven from a button. The rest of the UI uses about 110 hard-coded
  pixel sizes, so a CSS-based scale would be a refactor. Deferred 2026-10-02 as more work than the terminal control.

- **Fix Mac window restoration across display scales.** On a Mac whose displays use different scaling (for example a
  Retina laptop and a non-Retina monitor), the desktop app can reopen at the wrong size or position and save that wrong
  geometry again on close. The saved size is converted with the new window's scale and the position through the current
  scale (`crates/farhelm-ui/src/desktop/window_state.rs`). SPEC_impl.md's desktop window-persistence paragraph already
  says a real macOS display check is still needed. This needs work and verification on a real Mac with mixed-scale
  displays; do not fix it blind. Deferred in review-feedback triage on 2026-10-11
  (`window-restoration-uses-wrong-display-scale-macos.md`).

- **Let the Mac's supervisor outlive the desktop app.** The local supervisor now keeps running after the app quits, but
  the helm lives in the app, so until the app is reopened the agents' `farhelm` commands that need it (`farhelm spawn`,
  and the `farhelm agent` verbs) fail inside those sessions, and nothing restarts a supervisor that crashes while the
  app is closed or starts one at login. Consider a launchd agent like the Linux systemd units, so that those commands,
  and the skills and command-line features built on them, keep working with the app closed. Deferred 2026-10-04 while
  planning `plans/queue/hook-report-files.md`, which keeps conversation tracking working without a supervisor and states
  this gap in the spec as expected for now.

- **Keep a nested Grok from erasing the session's conversation switch.** A `grok` started inside a Grok session can make
  Restart resume the wrong conversation. Here's the sequence. You are in conversation A in a Grok session and run
  `/new`, which starts conversation B, and Grok's hook saves a report saying "this session is now in B". Before Farhelm
  reads that report, the session's Grok starts another `grok` (through its shell tool, say). That second `grok` runs
  your configured Farhelm hooks too, because Grok's hooks live in its configuration rather than on the launch's command
  line, and its hook saves a report for the same session that replaces the first one. Farhelm then reads the second
  report, sees that it comes from a `grok` nested below the session's own, and refuses it, which is correct, but B's
  report is already gone. The session stays on A, and Restart resumes A although you were in B. In a brand-new session
  whose first report was the one replaced, nothing is recorded, and Restart stays unavailable. Farhelm never records the
  nested `grok`'s conversation; the failure is a stale or missing conversation, never a wrong one.

  The window is short while the supervisor runs, since it reads reports every two seconds, but covers the whole outage
  while it does not (on the Mac, while the app is closed). It exists because each session keeps only its newest unread
  report (Grok keeps two: the one naming the conversation and the newest per-prompt one), and the check that refuses a
  nested report runs when Farhelm reads the file, not when the hook writes it. Reporting over the supervisor's socket,
  before #1594, checked each report as it arrived, so a refused report could never displace a good one. Claude, Codex,
  Pi and OMP are not affected: Farhelm adds their reporter to the one launch it builds, so a copy of them started inside
  a session reports nothing. Unverified: whether B's later per-prompt reports correct the session on their own; they
  only add detail to a conversation already named, so the session may stay on A until the next `/new` or a restart.
  Possible fixes: keep every report as its own file and apply them in order, so a refused report never replaces another
  (the plan ruled out only a capped queue, which would evict reports during a long outage); keep a slot per reporting
  process; or have the hook check its own process chain before writing. The accepted gap is recorded in SPEC_impl.md's
  "Report files". Found 2026-10-05 reviewing the `hook-report-files` plan's report.

- **Investigate harness-native notifications.** Claude Code and Codex can raise desktop notifications themselves, by
  writing terminal escape codes that a supporting terminal turns into a system notification. Claude Code picks the
  channel with its `preferredNotifChannel` setting (in `/config`; values such as `iterm2`, `ghostty`, `kitty` or
  `terminal_bell`) and fires when it needs the user: a permission prompt, or sitting idle waiting for input. Codex does
  it with `notifications = true` under `[tui]` in `~/.codex/config.toml`, and fires when a turn finishes and on approval
  requests. Inside Farhelm those codes reach the supervisor's own terminal emulator, which does nothing with them, so
  they end in Farhelm's terminal rather than a native terminal app. Investigate what Farhelm could do with them: surface
  them as client notifications, use them as a status signal, or nothing. Related to the ntfy.sh entry under Near term.

- **"Phone home" supervisors.** Allow supervisors to come up and register themselves with the helm, instead of being set
  up or pushed from the helm, so hosts show up dynamically as they come and go.

- **Tagging/capabilities for hosts and sessions.** Add tagging/capabilities so an agent can say "do X on a session on a
  host with capabilities Y" rather than naming specific hosts or sessions.

- **Trust zones.** A trust zone is scoped to a project and carries certain credentials; a session in a trust zone may
  spawn other sessions in the same zone, on hosts that allow that zone.

- **Consider a "project" concept that is not a session or a repo.** A project would be an object that lives across
  sessions and can span repositories, owning things that belong to the work rather than to any one session or checkout:
  its own identity, context, skills, a favicon, memory, and similar. Whether Farhelm has such a concept, what it owns,
  and how it is created and selected are all TBD.

- **Make the session list scale past its 500-session ceiling.** Each host lists at most 500 sessions and the helm's list
  across hosts at most 500, by design (SPEC.md, "The list is served and rendered WHOLE"), and the `session-limit-notice`
  plan only tells the user through the rail bell when a ceiling is hit. If people do run that many sessions, make the
  list scale instead. Recorded by the maintainer on 2026-11-18 while planning that notice.

## Unbucketized

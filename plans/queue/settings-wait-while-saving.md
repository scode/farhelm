# Goal: Simplify Settings around waiting on saves and finish surviving Settings follow-ups

Apply the maintainer's low-latency helm rule across Settings (and Add host's embedded recipe editor): make Close and
Escape wait for any in-flight Settings write or form-holding test, make credential editor Back and Cancel wait while
busy (matching Recipes), rip out the slow-helm concurrency machinery that those rules make dead (close-question watching
and refreshing, queued follow-up batches, Discard-absent close state, held-close test banners, detached credential save
tracking, the recipe leave-while-saving prompt, and Tensorlake Create SSH key background polling), and finish the
surviving Settings close, reveal, and footer follow-ups in one ordered stack.

This plan runs **after `custom-agent-delete-prompt.md`** (both touch `settings/custom_agents.rs`, `settings/tests.rs`,
and `goldens/settings.rs`, and this plan preserves the custom-agent inline Delete dismissal behavior that plan lands).

**Source TODO entries (all seven under `## Plan follow-ups` in `TODO.md`, removed in the last code PR):**

- `Show a failed hook setup save after Settings closes.`
- `Wait for credential saves after Back or Cancel.`
- `Reveal the credential editor when Save and Close fails.`
- `Name each custom-agent draft when closing Settings.`
- `Keep credential delivery reports visible when Settings closes.`
- `Bind Settings footer Save and Discard to the page and drafts shown.`
- `Stop Settings Save and Close when an independent save fails.` (retired as obsolete once Close/Escape wait removes
  concurrent close-question watching and queued follow-up batches)

## Acceptance criteria

1. **`SPEC.md` records the low-latency helm rule:** `SPEC.md` ("Settings") records the substance of the maintainer's
   rule: for Settings edits and saves, Farhelm treats the helm as a presumed low-latency, ordinarily reliable remote; it
   does not add asynchronous workarounds or surviving out-of-dialog notice machinery just to let users move on before a
   helm round trip settles, and waiting for a helm round trip before closing or leaving an editor is expected.
2. **Credential editor waits while busy and reveals when hidden:** In
   `crates/gui/farhelm-gui/src/settings/credentials/editor.rs`, Back and Cancel are unavailable while `busy()` and
   `leave()` returns while `busy()` (matching `settings/recipes/editor/`), eliminating detached-editor save tracking and
   fixing the lost delivery-failure report after Back. When Save and Close fails on a retained credential draft hidden
   behind "Resume editing credential" (`credentials.key_revealed`), `reveal_edit_failure` clears `key_revealed` only for
   that editor so its replacement secret and error are shown.
3. **Close and Escape wait for any in-flight Settings write or test (without a dialog-wide gate):**
   - Pressing Close or Escape while any Settings write or form-holding test is in flight returns `Dismissal::Stay` and
     remembers a `PendingClose` carrying `mark: Mark` (via `Mark::take(cx)`, refreshed on repeated Close/Escape) and a
     snapshot of `Credentials`'s `delivery_generation` counter. This wait covers: any form or editor Save, credential or
     custom-agent Delete (tracked via a separate busy predicate, never via `EditState::Saving`, so deleting never paints
     the unsaved dot or claims unsaved changes), a running Save/Save-and-Close loop, form-holding tests (`holds_save`
     evaluated across every loaded form — `self.ntfy` and both `TensorlakeKey` entities in Tensorlake and Credentials,
     NOT filtered through `edits()`, because Tests run on clean forms too), and **all five** immediate helm-preference
     switch/choice paths (`toggle_configured`, `save_choice` including `Choice::ShowWhatsNew` on `Section::App`,
     `turn_tunnels_off`, `tunnel_switch` -> `ask_consent` Allow, and `Tensorlake::toggle` for create-without-asking).
   - Do **not** add a dialog-wide busy gate locking section navigation, search, or unrelated sibling controls during
     saves. Footer Save stays busy during a loop as today, and immediate switches cause zero dialog-wide dimming or
     flicker.
   - Operations that stay **outside** the Close wait: Recipe Build/Rebuild (`202 Accepted` background work), Recipe
     Delete (`504` "carries on"), Containers Refresh/Delete, Tensorlake Sandboxes Refresh/Delete, Tensorlake Create SSH
     key, and the synchronous local App automatic-updates switch (`settings/app.rs`).
   - On settlement while `PendingClose` is set, always consume (`take()`) `PendingClose` regardless of outcome (closed,
     question opened, refusal revealed, user moved on, or declined for an interactive child) so a declined close never
     closes Settings after a newer dialog finishes. If `focus::has_moved_on(&mark, cx)` is true, drop the request
     without closing (this detects every real user navigation, search, edit, or click without firing on programmatic
     `dialog.show(...)` calls from custom-agent Add/Delete).
   - Otherwise, on the Close-wait resume path: on refusal (including a failed immediate switch or a failed
     ntfy/Tensorlake Test), cancel close and reveal the affected section with its inline error (extending
     `scroll_to_close_refusal` to scroll Notifications into view for both `save_refused()` and
     `matches!(tested, Some(Err(_)))`); else if `Credentials::delivery_generation()` advanced past the snapshot (meaning
     a credential replacement committed with missed hosts during this close wait — never an older already-seen report),
     cancel close and show `Section::Credentials` with `page_scroll` reset to `(0, 0)`; else re-enter `ask_to_close`
     (which finishes through `finish_close` preserving `layers::has_interactive_child`, or opens the unsaved-changes
     prompt if staged drafts are dirty).
4. **Sequential Save and Close with slow-helm concurrency machinery removed:**
   - The unsaved-changes close prompt opens only when nothing is in flight, so **Discard** is always offered.
   - Delete `watch_close_question`, `refresh_close_question`, `AfterBatch`, `Batch` generation/active-set/`paired_key`/
     `check_keys`, `ApprovedEdit::was_saving`, `close_save_held`, `CLOSE_SAVE_HELD`, `held_close_on_page`,
     `stage_held_close` (plus the `settings-close-save-held` golden scene), and the `recipes::Editor::save_focus()`
     accessor (keeping the `save_focus` field). Keep `Edit::holds_save`, `scroll_to_ntfy_refusal` /
     `scroll_to_close_refusal`, `layers::has_interactive_child`, `may_ask_tunnel_consent`'s `in_flight` probe,
     `if dialog.shown == Section::AddCustomAgent`, and `settings/refused.rs`.
   - Save and Close runs approved drafts strictly in sequence (`remaining: VecDeque<ApprovedEdit>`, `close: bool`,
     `has_delivery_warnings: bool`), checks for two different Tensorlake keys once up front, keeps
     `ApprovedEdit::permits` frozen-approval checks, joins an approved edit that already entered `EditState::Saving`
     mid-loop instead of re-sending it as a false refusal, stops cleanly and reveals the section if a queued edit's
     `holds_save(cx)` became `true` mid-loop, dismisses any open unanswered inline Delete confirmation on recipes and
     custom agents at save start, and stops on the first refusal revealing that page with its error.
   - When the sequential Save-and-Close loop finishes with no refusals: if `has_delivery_warnings` was set by a
     credential edit dispatched by **this loop** (never by reading an older report), show `Section::Credentials` with
     `page_scroll` reset to `(0, 0)`; otherwise call `finish_close` **only if `edits()` is empty** (if the user edited a
     draft mid-loop, stay open silently per `SPEC.md:2685` rather than re-popping the close question).
5. **Add host embedded recipe editor waits while saving, and Tensorlake Create SSH key drops background polling:**
   - In `settings/recipes/editor/leaving.rs`, `Editor::dismissal()` (called only by Add host at `hosts/add.rs:861`;
     Settings' recipe wait comes from PR 3) returns `Dismissal::Stay` while `saving`. Delete the leave-while-saving
     question, `dismiss_refused_leave`, the now-dead `Editor::leave_question` field and `LayerId` import, and the
     unreachable `Recipes::depart` Close handling (keeping the `Departure::Close` variant for Add host).
   - In `settings/tensorlake_ssh_key.rs`, delete the ~30-second timer-based re-read schedule and its timer tests. On any
     Create SSH key refusal, keep showing the helm's error verbatim and perform one `read_again`, keeping `TRY_AGAIN`
     after that single read finds no key so mixed-version older helms do not promise polling that no longer exists.
     Reword only the helm's 504 timeout text in `crates/farhelm-helm/src/tensorlake/mod.rs` to instruct selecting Create
     SSH key again, and update its helm and GUI tests.
6. **Surviving close/footer follow-ups:**
   - Name each dirty custom-agent draft in one `HostSentence` line in the close prompt (using `"Add custom agent"` for
     Add and each registration's display name, falling back to stored name/ID if externally deleted), and reveal a page
     only when one distinct `Section` is affected (staying on the current page when multiple sections are affected).
   - Key footer Save and Discard button IDs by `self.shown.key()` and bind `page_answers`, `save_page`, and
     `discard_page` with `prompt::ActionFrame` and `ApprovedEdit::permits` (deriving `PartialEq` on `ApprovedEdit` and
     clearing `page_frame` when `page_answers` is skipped).
7. **Specs, parity docs, goldens, and tests stay consistent at every PR:**
   - Each behavioral amendment to `SPEC.md`, `SPEC_impl.md`, and `docs/gpui/parity/settings-and-hosts.md` lands in the
     same PR that changes that behavior (only the general low-latency helm rule lands in PR 1).
   - Targeted GUI and helm tests cover every changed and preserved behavior (including tests outside `settings/` that
     close Settings with an unanswered preference save, such as
     `session_pane/trust_band_tests.rs::the_band_follows_only_the_stored_dismissal`); every PR compiles without
     dead-code warnings and passes its tests on its own; only `settings-close-save-held` is retired
     (`settings-notifications-close-refused`, `settings-custom-agent-busy-save`, `settings-custom-agent-save-refused`,
     and `settings-credentials-key-conflict` are kept and re-captured only if visuals change).

## Decisions already made

- **Maintainer's `SPEC.md` rule (preserve the substance in `SPEC.md` "Settings"):**
  > For the purposes of settings edits/saves, we treat the Helm as a presumed low latency infallible remote. We do not
  > add asynchronous flows to settings editing flows to account for helm being far away. It's okay to make the user wait
  > for a helm round trip before they can move on after "save" or similar operation. We do not add complexity just to
  > allow the user to move on if they don't have solid low latency connectivity to their helm.
- **No dialog-wide busy gate:** Settled after dual fresh-context plan reviews (Neon + Opus). Keep Wait-on-Close/Escape,
  editor Back/Cancel unavailable while busy, and sequential Save-and-Close — without adding a dialog-wide gate over
  section navigation, search, or sibling controls. That deletes the exact same slow-helm machinery while avoiding focus
  and control-gating regressions.
- **Consolidate into one plan and abandon `settings-hook-save-notice` and `settings-close-save-hardening`:** Covers all
  seven Settings follow-ups in one ordered stack sequenced after `custom-agent-delete-prompt.md`, retiring
  `Stop Settings Save and Close when an independent save fails.` as obsolete.
- **Save and Close UX:** Keep Settings waiting and reveal the failing page with its inline error (keeping current
  `SPEC.md` behavior rather than switching to `prompt::ask_fallible`).
- **Immediate switches:** Hold Close/Escape only, with zero dialog-wide visual dimming or flicker.
- **Add host embedded recipe editor:** Covered by the same wait-while-saving rule; delete the leave-while-saving prompt.
- **Credential delivery warnings on Close:** Keep Settings open on `Section::Credentials` (scrolled to top) when a
  replacement commits with missed hosts during Close or Save-and-Close (if no later draft failed, and never triggered by
  an already-seen report).
- **Tensorlake Create SSH key 504 polling:** Rip out the ~30-second background polling loop and reword the helm's 504
  text to tell the user to select Create SSH key again.
- **Custom-agent draft names & footer binding:** Keep triage's small scope (one `HostSentence` line + reveal only for
  one section; section-keyed footer IDs + `ActionFrame` + `ApprovedEdit::permits`, not a custom primary-down/release
  pointer state machine).
- **Review gate:** Single fresh-context Opus 5.5 agent at high effort with the general review charter below.

## Preserved invariants (do not rip out)

1. Checkout compare-on-save (`checkout_field.rs` stored base, 409 handling, lock across save/refetch,
   `read_generation`).
2. Read-only fields while saving and busy (not disabled) Save keeping keyboard focus (`SPEC_impl.md:1174-1181`).
3. Unsaved drafts preserved across section switches, multi-form close prompt, and custom-agent `sync_custom_agents`
   "Register again" draft retention on external deletion.
4. Frozen close-prompt approvals (`ApprovedEdit::permits`, `EditVersion`, minus `was_saving`) and `SPEC.md:2684-2686`
   ("New or changed drafts stay in Settings for a fresh action" and `layers::has_interactive_child` keeping newer
   dialogs such as tunnel consent open).
5. Up-front check for two conflicting Tensorlake key drafts and two key forms over one shared `TensorlakeAccount`.
6. Credential delivery-failure report on Credentials overview and per-host warnings.
7. Tensorlake leftover-sandbox discovery rounds and old-key guard (handled alongside `tensorlake-sandbox-key-consent`).
8. Operations outside Close-wait: Recipe Build/Rebuild (`202 Accepted` + push reconciliation), Recipe Delete (`504`
   "carries on" and "Delete anyway", pinned by `a_close_while_the_delete_is_out_closes_at_once`), Containers
   Refresh/Delete, Tensorlake Sandboxes Refresh/Delete, Tensorlake Create SSH key, and the synchronous local App
   automatic-updates switch.
9. Push-before-reply ordering guards (`custom_agents.rs` `saw_saved` and delete-owner guard, `file_types.rs` deferred
   `follow_store`, `ntfy.rs` deferred refill), `if dialog.shown == Section::AddCustomAgent`, and
   `scroll_to_ntfy_refusal` / `scroll_to_close_refusal`.
10. Focus moving to Copy after Create SSH key (`tensorlake_tests.rs:890-954`) and create-without-asking showing only the
    acknowledged choice (billing safety).
11. Codex `auth.json` native file-picker race guard (`credentials/editor.rs`).
12. Shared `operations.rs`, `settings/refused.rs`, `may_ask_tunnel_consent`'s `in_flight` probe, and
    `file_types_tests.rs:254-270` (because ~16 writers outside Settings plus unlocked switches share
    `OpKey::preferences()`).
13. Save and Close dismissing an unanswered inline Delete confirmation on both recipes (`recipes::Editor::save` clearing
    `delete_question`) and custom agents (from `custom-agent-delete-prompt.md`).

## Implementation outline and requirement sources

- **User request & decisions:** The seven `TODO.md` entries listed above plus the maintainer's low-latency helm
  simplification request and settled Q&A/review decisions above.
- **Binding repository constraints:** `SPEC.md` ("Settings", "Credentials", "Errors and diagnostics", "One GUI at a
  time"), `SPEC_impl.md` ("Questions: the prompt", Settings/Credentials/Tensorlake/Recipes sections), and
  `docs/gpui/parity/settings-and-hosts.md`.
- **Planner-proposed implementation choices (two rounds of dual Neon + Opus reviews, 5 ordered PRs):**
  - **PR 1 (`01-spec-low-latency-helm`, `docs:`):** Record only the maintainer's general low-latency helm rule in
    `SPEC.md` ("Settings") so `SPEC.md` never contradicts `main` before the code PRs land.
  - **PR 2 (`02-credential-editor-waits-and-reveal`, `fix:`):**
    - In `settings/credentials/editor.rs`, make Back and Cancel unavailable while `busy()` and guard `leave()` against
      `busy()`. Keep `if started.is_ok() { self.error = None; } cx.notify();` in `started()`.
    - Remove the dead keep-alive loop in `settings/editing.rs::edits()` (`381-387`) and update the comment on
      `Batch::active` (keeping the `active` field until PR 3 removes `Batch`), delete the dead own-editor delete guard
      in `settings/credentials.rs:400-403` and drop the unused `editor: EntityId` parameter from `close_editor` and its
      3 callers in `editor.rs` (keeping `undelivered` populated before closing the editor), update the stale doc
      comments at `credentials.rs:384-388` and `credentials/editor.rs:373`, and remove
      `a_delete_answered_after_back_leaves_a_newer_editor_open` (`settings/credential_tests.rs:864`) and its parity-doc
      citation (`docs/gpui/parity/settings-and-hosts.md:426`).
    - In `settings/editing.rs::reveal_edit_failure`, clear `credentials.key_revealed` when `Edit::Credential(ed)`
      matches `credentials.editor()`.
    - Update `SPEC_impl.md:6375-6376` and `docs/gpui/parity/settings-and-hosts.md:1804-1806`; add focused tests and
      verify light/dark captures for the revealed editor.
  - **PR 3 (`03-settings-close-wait-and-sequential-save`, `fix:` — leave the matching changelog fragment per
    `AGENTS.md`, "Changelog fragments"):**
    - Define one close-wait busy predicate on `SettingsDialog`: any `Edit` in `EditState::Saving`, `holds_save`
      evaluated directly across every loaded form (`self.ntfy` and both `TensorlakeKey` entities in Tensorlake and
      Credentials — NOT filtered through `edits()`, so Tests on clean forms are caught),
      `credentials::Editor::deleting`, `custom_agents::Editor::deleting`, a running save loop, or
      `in_flight(&OpKey::preferences())` (covering all five immediate-save sites: `toggle_configured`, `save_choice`
      including `Choice::ShowWhatsNew`, `turn_tunnels_off`, `ask_consent` Allow, and `Tensorlake::toggle`).
    - **Settlement ordering (avoid `Operations::settle` / `cx.defer` FIFO races):** In `operations.rs:1024-1030, 1088`,
      `Operations::settle` removes the key and calls `cx.notify()` _before_ `on_settled` runs, and GPUI drains
      `cx.defer` FIFO (so an observer's `cx.defer` would run before `on_settled`'s `cx.defer`, such as
      `TensorlakeKey::empty_later` and `SaveReceipt::finish`). Therefore, drive Settings-owned close-resume from each
      operation's own completion path (deferred _after_ state updates like `empty_later`, `answered`, `follow_store`,
      and refusal storage): all five immediate-switch callbacks, `credentials::Editor` save/delete,
      `custom_agents::Editor` save/delete, `recipes::Editor` save, `NtfyAlerts::send_test`, and
      `TensorlakeKey::test`/`clear` (observing `self.ntfy` and both `TensorlakeKey` entities as needed). Also trigger
      idempotent close-resume in `sync_custom_agents` when a push-before-reply removes a deleting custom-agent editor
      (`custom_agents.rs:175, 484-497`) so Close never hangs waiting on a removed editor. Only use an
      `operations::Operations` (`operations.rs:937`) observer to wake close-resume when an _external_
      `OpKey::preferences()` writer settles (never preempting a Settings-owned completion callback).
    - **Pending-close lifecycle and delivery-report freshness:** Store `close_waiting: Option<PendingClose>` carrying
      `mark: Mark` (`Mark::take(cx)`, refreshed on repeated Close/Escape) and `delivery_generation: u64` (a counter on
      `Credentials` bumped in `close_editor` whenever `!undelivered.is_empty()`). When the Close-wait resume runs,
      always consume (`take()`) `close_waiting` regardless of outcome; if `focus::has_moved_on(&mark, cx)` is true,
      return without closing. Otherwise: on refusal (save, delete, immediate switch, or failed ntfy/Tensorlake Test),
      reveal the affected section (extending `scroll_to_close_refusal` to also scroll Notifications when
      `matches!(tested, Some(Err(_)))`); else if `credentials.delivery_generation() > snapshot`, show
      `Section::Credentials` with `page_scroll` reset to `(0, 0)`; else re-enter `ask_to_close` (which finishes via
      `finish_close` preserving `has_interactive_child`, or opens the unsaved-changes prompt if drafts are dirty).
    - Now that `credentials::Editor::deleting` holds Close, remove the dead `OpKey::credential` `Refused` line in
      `credentials/editor.rs` (keeping error clearing in `started()`).
    - Replace `Batch` / `AfterBatch` / `save_generation` / `active` / `paired_key` / `check_keys` /
      `watch_close_question` / `refresh_close_question` / `ApprovedEdit::was_saving` / `close_save_held` /
      `CLOSE_SAVE_HELD` / `held_close_on_page` / `stage_held_close` with a plain sequential loop
      (`remaining: VecDeque<ApprovedEdit>`, `close: bool`, `has_delivery_warnings: bool`) checking conflicting
      Tensorlake keys once up front, keeping `Edit::save`'s `already_saving` join (`editing.rs:289, 689-695`) for a
      queued edit that started saving mid-loop, stopping cleanly and revealing the section if a queued edit's
      `holds_save(cx)` became `true` mid-loop, clearing any open `delete_question` in `recipes::Editor::save` (and
      preserving custom-agent Delete prompt dismissal), keeping `scroll_to_close_refusal`, and setting
      `has_delivery_warnings` only from completions of credential edits dispatched by **this loop**. On loop completion
      with no refusals: if `has_delivery_warnings`, show `Section::Credentials` with `page_scroll` reset to `(0, 0)`;
      else call `finish_close` only if `edits()` is empty (otherwise stay open silently per `SPEC.md:2685`).
    - Delete the `recipes::Editor::save_focus()` accessor (`recipes/editor.rs:469-471`, keeping the `save_focus` field)
      so PR 3 compiles cleanly without dead-code warnings.
    - Update `SPEC.md:2673-2681`, `SPEC.md:5111-5122`, `SPEC_impl.md:3857-3858`, `6249-6250`, `6942-6944` (keeping
      `SPEC.md:2684-2686` for frozen approvals and `has_interactive_child`, and keeping pending switch saves in
      `SPEC.md:2674`), and update the affected `docs/gpui/parity/settings-and-hosts.md` entries (`255`, `299` including
      fixing the stale test name, `311`, `341-347`, `531-533`, `1251-1252`, `1287-1295`, `1599-1602`, the Settings half
      of `1742-1755`, and `1810-1812`; leave `247` alone because
      `switching_sections_keeps_a_pending_save_and_a_failure_line` is kept, and leave the Add host half of `1742-1747`
      for PR 4).
    - Grep the whole repo for tests that close Settings with an unanswered preference save, and rewrite all
      contradicting concurrency tests: `session_pane/trust_band_tests.rs:617`
      (`the_band_follows_only_the_stored_dismissal`), `settings/tests.rs` (`1637-1673`, `3425`, `3719-3766`,
      `3836-3903`, `4583-4600`), `settings/credential_tests.rs:1050`, `settings/tensorlake_tests.rs:289-345`, and
      `settings/recipe_leave_tests.rs:477-591` (keeping `a_close_while_the_delete_is_out_closes_at_once` at `:524`,
      `switching_sections_keeps_a_pending_save_and_a_failure_line` at `settings/tests.rs:1589`, and
      `file_types_tests.rs:254-270`). Add focused tests for Close waiting on immediate switches (including hook opt-out,
      tunnel toggle, and create-without-asking), clean-form ntfy/Tensorlake Tests, credential/custom-agent Delete, and
      fresh vs already-seen credential delivery warnings on both Save-and-Close and Close-during-editor-save. Retire
      `settings-close-save-held` across `goldens/mod.rs`, `goldens/settings.rs`, `docs/gpui/screenshot-scenes.json`, and
      `tests/goldens.txt`; keep `settings-notifications-close-refused`, `settings-custom-agent-busy-save`,
      `settings-custom-agent-save-refused`, and `settings-credentials-key-conflict`.
  - **PR 4 (`04-recipe-wait-and-ssh-key-no-poll`, `fix:`):**
    - In `settings/recipes/editor/leaving.rs`, `settings/recipes/editor.rs`, and `settings/recipes.rs`, make
      `Editor::dismissal()` return `Dismissal::Stay` while `saving` (matching Add host's existing `Stay`-while-busy
      behavior in `hosts/add.rs:859-882`, or optionally recording `leaving = Some((Departure::Close, Mark::take(cx)))` —
      log whichever choice as a `DECISION`), always offer Discard in the leave question, delete the leave-while-saving
      branch, delete now-dead `dismiss_refused_leave` (`leaving.rs:156-166` and its single call at `editor.rs:588` ONLY
      — keep `:586` `editor.leaving = None;` and `:587` `editor.error = Some((refusal, sent));`), delete now-dead
      `Editor::leave_question` (`editor.rs:293, 363`, `leaving.rs:140-146, 219`) and `use crate::layers::LayerId;`
      (`editor.rs:64`), turn `Recipes::depart`'s unreachable `Departure::Close` arm into a documented no-op (keeping
      `Departure::Close` for Add host), and drop `Recipes::layer` (`LayerId` import only; keep `use crate::layers;` for
      `layers::beside_edge` at `recipes.rs:1056`).
    - In `settings/tensorlake_ssh_key.rs`, delete the ~30-second re-read timer schedule (`RECHECKS`, `schedule_recheck`,
      `recheck`, `shown_again`, `next_recheck`, and glue in `tensorlake.rs` and `settings.rs`) while keeping
      `SshKey::created` showing `Err(reason)` verbatim plus one `account.read_again(cx)` and keeping `gave_up` (minus
      the `self.next_recheck.is_none()` clause) so `TRY_AGAIN` still renders after that single read finds no key on
      mixed-version helms. Reword `crates/farhelm-helm/src/tensorlake/mod.rs:501-505` to instruct selecting Create SSH
      key again, and update `crates/farhelm-helm/src/tensorlake/tests.rs:1700`, `SPEC_impl.md:302-306`, and
      `docs/gpui/parity/settings-and-hosts.md` (Add host half of `1742-1747` and `1821-1839`). In
      `settings/tensorlake_tests.rs`, rewrite `a_refused_create_reads_the_account_now_and_on_the_next_showing` (`:538`)
      into focused coverage for the updated 504 wording shown verbatim, single account read, `TRY_AGAIN` when that read
      finds no key (and hidden on failed read), second-Create recovery, and no read on next showing; delete the other
      seven timer/`shown_again` tests (`:606-859`) and any now-unused helpers (`CARRIES_ON`, `refuse_create`,
      `one_account_read`), preserving `:890-954`.
  - **PR 5 (`05-close-names-and-footer-binding`, `fix:`):**
    - In `settings/editing.rs`, name each dirty custom-agent draft (`"Add custom agent"` or the registration's display
      name, falling back to stored name/ID if externally deleted) in one `HostSentence` line, and reveal a page only
      when one distinct `Section` is affected.
    - Derive `PartialEq` on `ApprovedEdit`, key footer Save/Discard button IDs by `self.shown.key()` (updating the ~50
      footer button selector callsites across `settings/tests.rs`, `tensorlake_tests.rs`, `recipe_tests.rs`,
      `credential_tests.rs`, `tensorlake_sandbox_tests.rs`, and `file_types_tests.rs`), and change `page_answers` to
      `(&mut self, window: &Window, cx: &Context<Self>)` to capture `prompt::ActionFrame` (clearing it when
      `page_answers` is skipped) and validate with `ApprovedEdit::permits` in `save_page`/`discard_page`.
    - Remove all seven covered entries from `TODO.md` (and leave the matching changelog fragment per `AGENTS.md`).

## Scope reassessment

Before implementing a substantial departure from the outline, run a fresh-context review at the executing session's own
model and effort with this charter:

> Identify where this plan does more than the user needs, and where it duplicates a mechanism the repository already
> has. For each substantial new mechanism, explain which requirement makes it necessary and whether an existing facility
> or narrower supported behavior would suffice. Where the plan builds something that does substantially the same job as
> an existing mechanism, say whether that mechanism could be reused as it stands or generalized to serve both uses, or
> whether keeping them separate is better because sharing would blur two different contracts, and what generalizing
> would cost; never propose generalizing beyond the uses the plan actually has. Weigh the two against each other: a plan
> is not simpler for copying a mechanism, and not better for generalizing one nobody else needs. Propose the smallest
> coherent design that avoids needless duplication. Challenge planner-authored requirements; identify any simplification
> that would change an explicit user requirement or repository constraint, and mark any reuse-versus-duplication choice
> that is a close judgment call rather than clear-cut. Report concrete findings and alternatives, or say no unnecessary
> complexity or duplication was found. Do not manufacture objections, add safeguards to justify an unnecessary
> subsystem, or add abstraction to justify a generalization. Write findings to the supplied private file; change no
> product files or VCS state.

Supply the request, decisions, outline, current diff, and proposed departure. Repeated corrective reviews of the same
component also require reassessing its necessity before another repair round. Give same-model sub-agents bounded units
within the outline; unapproved new mechanisms are outside their task.

## The checkout

This goal runs inside the checkout the executing session runs in, identified by GitHub repository `scode/farhelm` (the
nearest Git worktree or jj workspace containing the working directory; never identified by path). Confirm first that the
nearest checkout has a remote pointing at `scode/farhelm`; if not, stop before the resume protocol, write nothing, and
report the mismatch. Every project file path in this plan is repo-relative.

## Resume protocol

Right after the checkout check, resolve the plan's record with
`cargo xtask records path plan/settings-wait-while-saving` (`plans/.agents/outside-files.md`, "Files outside the
repository") and invoke the `agent-resumeable` skill with that directory's `log.md`. If `log.md` already exists, read it
in full and resume where the previous session left off — cross-checking against GitHub and jj state per
`plans/.agents/executing.md`, "Executing one plan" — rather than starting over. If it does not exist, this is a fresh
start.

## Execution mode

Explicit **no-workhorse mode**: do not send any unit of your own decomposition to a weaker model or lower effort than
your own, and do not invoke, load, or follow `scode-galaxy-brain` or `scode-model-routing` at any point (if
`scode-galaxy-brain` is active when the run starts, stop it). Same-model sub-agents at your own model and effort are
allowed whenever they finish work sooner without costing quality, using separate worktrees or jj workspaces if
concurrent writers might conflict. The review gate keeps the explicit Opus 5.5 high demand below, scope reassessment
runs at your own model and effort, and the required sub-agents listed in `plans/AGENTS.md`, "Sub-agents this file
requires" (resume check, cold reads of Blocked questions and reports), are exempt from any no-delegation restriction.
Launch any sub-agent whose model is unavailable natively through `scode-harness-shellout`.

## Resource watchdog

Right after the resume protocol and before any delegation, start the `scripts/watchdog` Deno script from the installed
`scode-build-goal` skill directory (resolved with `realpath` at run time) to monitor memory and disk for the rest of the
run:

- **Status file:** `farhelm-plan-settings-wait-while-saving-watchdog.json` in the parent directory of the executing
  checkout (derived per `scode-build-goal`, "Placing the files", never inside the `plan/settings-wait-while-saving`
  record directory).
- **Watched paths (60s interval, default alert thresholds):** the executing checkout root, `$HOME`, `/tmp`, the parent
  directory holding the status file, the session's scratch directory, and any temporary worktree or build directory
  created on another filesystem.
- **Verification & lifecycle:** Verify the delivery mechanism on throwaway status files in scratch space
  (`--interval 5`, `--synthetic-alert`, killed pid, `--stall-after 2`) before arming the real monitors (Claude Code dual
  Monitors; Codex `wake-on-exit`; plain background fallback with `watchdog check` before launches and at boundaries if
  neither is available). Run `watchdog check --status <file>` before every new workload launch and at natural
  boundaries. Act immediately on `ALERT`/`WORSE`/`STALE`/`ERROR` by pausing new launches and freeing run-owned build
  artifacts; stop owned monitors at close.

## Stack, PR discipline, and repository conventions

- Build a linear stack of bite-sized draft PRs on top of `main@origin` (or this plan's own open PRs when resuming, never
  another plan's PRs) using the `jjstack` skill, set up per `plans/.agents/executing.md`, "Executing one plan".
- Name bookmarks `plan/settings-wait-while-saving/<nn>-<short-name>`. Keep PRs as drafts; never mark ready and never
  merge.
- Restructure existing PRs in the stack (including ones from an earlier run) instead of stacking corrections on top.
- Never edit anything under `plans/`.
- The last code PR removes all seven covered entries from `TODO.md`.
- Follow `AGENTS.md` ("Conventional Commits", "Changelog fragments", "Tests and validation") and
  `.agents/validation.md`, `.agents/test-evidence.md`, and `.agents/test-authoring.md` (supply
  `.agents/test-authoring.md` verbatim to the reviewer when tests or fixtures change).

## Review gate

Before finishing any PR in the stack, run a review by a single fresh-context **Opus 5.5** agent at **high** effort
(explicit demand for model and effort; launch natively or via `scode-harness-shellout` without hardcoding a CLI command
here). Give the reviewer the repo root, the commit range or bookmark to review, the private findings file path in
scratch space, and `.agents/test-authoring.md` verbatim if tests changed, with this charter:

> Review the PR's changes for general correctness (bugs, unhandled cases, broken invariants, wrong behavior against the
> goal's acceptance criteria), design (whether the shape of the change fits the codebase and the goal, and whether a
> simpler shape would), and idiomatic code for the languages involved; report findings to the named file, most severe
> first, with the file, the quoted code, what is wrong, and the smallest fix; edit nothing and touch no VCS state.

Address what it finds before moving on.

## Decision logging and unattended fallback

Log consequential design choices in the working log as `DECISION` with alternatives considered. Resolve routine
implementation forks within agreed scope and log them; remove unnecessary planner-invented machinery when agreed
behavior and repository constraints still hold.

Never ask the maintainer anything in conversation — not a direct question, not a harness question tool prompt, not an
optional offer or closing question (`plans/AGENTS.md`, "Unattended means no questions"). Every fork is either decided
within scope and logged as `DECISION`, or — when a material scope expansion, weakened guarantee, or maintainer-only
product choice cannot be resolved from the plan, its `## Decisions`, and the specs — blocked per
`plans/.agents/executing.md`, "Executing one plan", step 10. Things the maintainer should know and optional follow-ups
go into the report.

## Done criterion

Every PR in the stack exists on GitHub, has passed its Opus 5.5 high review gate, removes all seven covered `TODO.md`
entries in the last code PR, and satisfies the latest `## Decisions` entry (if any). Delivering the report is the
executor's closing step, not part of this goal.

## Release note

No release note needed.

# Triage outcomes

## orphan-cleanup-overlay-forgets-failed-discovery.md

- Outcome: `fix code`.
- Assessment: helm tensorlake/orphans.rs ~121–133 returns Ok(unfinished rows) on a failed discovery. GUI
  settings/tensorlake_sandboxes.rs ~219 `answered` replaces `rows` and clears `failure`. Basis: SPEC.md (~556): "A
  failed read shows its reason and keeps earlier rows rather than claiming an empty result." Fix: keep the earlier rows
  and show the discovery failure beside the cleanup rows. There is a small shape choice: the helm signals the failure
  separately, or the GUI merges.
- Decision: recorded under the clearly-settled rule (`review_feedback_queue/.agents/triage.md`, "Preparing each item")
  without asking (fewest-questions triage, 2026-11-21, against main ca62397de). The maintainer can revise it.
- Completion criteria: fix the code so it matches the quoted rule, with a test where practical. Size gate: about 100
  lines of diff (code, tests and docs, not counting this ledger, the queue removal or a changelog fragment); over that,
  execution blocks and brings the item back to the maintainer. Remove the feedback file and its index entry.
- Execution: planned in `plans/queue/triage-tensorlake.md`.

## omitted-session-row-drops-live-tabs.md

- Outcome: `fix code`.
- Assessment: terminal/set.rs `follow_tabs` (~528–537) maps a missing row to an empty tab list
  (`listed.unwrap_or_default()`), dropping tabs and falling back to the agent. Its own comment says "A missing row is
  still waiting for its first list." The cut case needs one host with more than 500 sessions, far above the stated "tens
  of sessions" scale. Basis: SPEC_impl.md (~402): "a host or status filter, or the cap, can leave that session out of
  the list while the selection and its terminal stay". Fix: leave the tabs untouched when the row is missing.
- Decision: recorded under the clearly-settled rule (`review_feedback_queue/.agents/triage.md`, "Preparing each item")
  without asking (fewest-questions triage, 2026-11-21, against main ca62397de). The maintainer can revise it.
- Completion criteria: fix the code so it matches the quoted rule, with a test where practical. Size gate: about 100
  lines of diff (code, tests and docs, not counting this ledger, the queue removal or a changelog fragment); over that,
  execution blocks and brings the item back to the maintainer. Remove the feedback file and its index entry.
- Execution: planned in `plans/queue/triage-gui-misc.md`.

## session-delete-shortcut-ignores-composition.md

- Outcome: `fix code`.
- Assessment: actions.rs module docs (~93–101) say the composition guard "is not built here", and on macOS "a listed
  shortcut typed mid-composition still runs". The handler checks no marked text. The terminal tracks marked text
  (attached.rs ~558). Basis: SPEC.md "Keyboard shortcuts" (~3738): "Both session shortcuts work from terminal focus
  without sending their chord to the program and ignore repeat and composition." SPEC_impl.md (~1285): refusing
  mid-composition "falls to the action's handler, which can ask the focused input whether it is composing." Fix: the
  Delete and Rename handlers refuse while the focused input has marked text.
- Decision: recorded under the clearly-settled rule (`review_feedback_queue/.agents/triage.md`, "Preparing each item")
  without asking (fewest-questions triage, 2026-11-21, against main ca62397de). The maintainer can revise it.
- Completion criteria: fix the code so it matches the quoted rule, with a test where practical. Size gate: about 100
  lines of diff (code, tests and docs, not counting this ledger, the queue removal or a changelog fragment); over that,
  execution blocks and brings the item back to the maintainer. Remove the feedback file and its index entry.
- Execution: planned in `plans/queue/triage-gui-misc.md`.

## wayland-terminal-paste-ignores-file-list.md

- Outcome: `fix code`.
- Assessment: vendor/gpui-pre-linux wayland/clipboard.rs `payload_mime_type` picks only image or text types.
  `text/uri-list` is defined (FILE_LIST_MIME_TYPE) but only used for drag-and-drop and offering. Basis: SPEC.md
  "Attachments" (~3512): pasted content "is classified by flavor ... file references first ... A file reference means an
  actual file object on the clipboard". Fix: read text/uri-list in the vendored Wayland clipboard as a file item. This
  is a vendor patch; gpui-pre-linux is not on the frozen-vendor list.
- Decision: recorded under the clearly-settled rule (`review_feedback_queue/.agents/triage.md`, "Preparing each item")
  without asking (fewest-questions triage, 2026-11-21, against main ca62397de). The maintainer can revise it.
- Completion criteria: fix the code so it matches the quoted rule, with a test where practical. Size gate: about 100
  lines of diff (code, tests and docs, not counting this ledger, the queue removal or a changelog fragment); over that,
  execution blocks and brings the item back to the maintainer. Remove the feedback file and its index entry.
- Execution: planned in `plans/queue/triage-gui-misc.md`.

## linux-ctrl-f-leaves-settings-field-at-end.md

- Outcome: `fix code`.
- Assessment: Confirmed. gpui-base 0.9.1 `movement.rs` `right()` propagates when a single caret sits at the end of the
  text. `vendor/gpui-pre/src/window.rs` ~6649 then tries the next matching binding, which is Settings' `SearchSettings`
  (settings.rs ~265). Basis: SPEC.md ~2467-2468: "Ctrl+F on Linux moves focus there from Settings, except where a Linux
  text field uses Ctrl+F to move its caret." The fix follows directly: a Linux field must consume Ctrl+F even when the
  caret cannot move.
- Decision: recorded under the clearly-settled rule (`review_feedback_queue/.agents/triage.md`, "Preparing each item")
  without asking (fewest-questions triage, 2026-11-21, against main ca62397de). The maintainer can revise it.
- Completion criteria: fix the code so it matches the quoted rule, with a test where practical. Size gate: about 100
  lines of diff (code, tests and docs, not counting this ledger, the queue removal or a changelog fragment); over that,
  execution blocks and brings the item back to the maintainer. Remove the feedback file and its index entry.
- Execution: planned in `plans/queue/triage-gui-misc.md`.

## untagged-sandbox-leftover-labelled-another-helm.md

- Outcome: `fix code`.
- Assessment: Confirmed. orphans.rs ~151 and ~210 use `creator(&name) != Some(tag)`, which is true when `creator`
  returns None. Basis: SPEC.md ~551: "**from another helm** when its name carries another helm's tag." The fix shows the
  label only for `Some(other)`.
- Decision: recorded under the clearly-settled rule (`review_feedback_queue/.agents/triage.md`, "Preparing each item")
  without asking (fewest-questions triage, 2026-11-21, against main ca62397de). The maintainer can revise it.
- Completion criteria: fix the code so it matches the quoted rule, with a test where practical. Size gate: about 100
  lines of diff (code, tests and docs, not counting this ledger, the queue removal or a changelog fragment); over that,
  execution blocks and brings the item back to the maintainer. Remove the feedback file and its index entry.
- Execution: planned in `plans/queue/triage-tensorlake.md`.

## sandbox-termination-poll-exceeds-phase-deadline.md

- Outcome: `fix code`.
- Assessment: Confirmed. `tensorlake/hosts.rs` `wait_until_terminated` checks the 30 s deadline only after each lookup,
  and each lookup gets its own 20 s `REMOVAL_CALL_TIMEOUT`. Basis: SPEC_impl.md, Tensorlake hosts: "it polls until
  Tensorlake reports it terminated (at most 30 seconds, ...)". The fix is to bound each poll by the remaining time (or
  wrap the loop in the deadline). There is effectively one shape.
- Decision: recorded under the clearly-settled rule (`review_feedback_queue/.agents/triage.md`, "Preparing each item")
  without asking (fewest-questions triage, 2026-11-21, against main ca62397de). The maintainer can revise it.
- Completion criteria: fix the code so it matches the quoted rule, with a test where practical. Size gate: about 100
  lines of diff (code, tests and docs, not counting this ledger, the queue removal or a changelog fragment); over that,
  execution blocks and brings the item back to the maintainer. Remove the feedback file and its index entry.
- Execution: planned in `plans/queue/triage-tensorlake.md`.

## sandbox-status-diagnosis-bypasses-cleaning.md

- Outcome: `fix code`.
- Assessment: Confirmed on main. `error_details` and `termination_reason` from successful status JSON go straight into
  displayed errors (`tensorlake/lifecycle.rs` ~225, `tensorlake/hosts.rs` 97/476/515, `build.rs` ~1535), bypassing
  `http.rs` cleaning, which is applied only to non-2xx bodies. Basis: SPEC_impl.md, Tensorlake account: "Server messages
  are kept for display, bounded, whitespace-collapsed, stripped of presentation-unsafe characters, and have the key
  replaced should a server echo it." The fix is to route these display strings through the existing cleaner with the
  key. Caveat: this counts `error_details` as a "server message", which I consider a plain reading. If the coordinator
  disagrees, move it to DECIDE together with provider-error-cleaning-can-miss-transformed-secret.md.
- Decision: recorded under the clearly-settled rule (`review_feedback_queue/.agents/triage.md`, "Preparing each item")
  without asking (fewest-questions triage, 2026-11-21, against main ca62397de). The maintainer can revise it.
- Completion criteria: fix the code so it matches the quoted rule, with a test where practical. Size gate: about 100
  lines of diff (code, tests and docs, not counting this ledger, the queue removal or a changelog fragment); over that,
  execution blocks and brings the item back to the maintainer. Remove the feedback file and its index entry.
- Execution: planned in `plans/queue/triage-tensorlake.md`.

## unreadable-sandbox-ssh-key-reports-ready.md

- Outcome: `fix code`.
- Assessment: Confirmed. `tensorlake/ssh_key.rs` `read` checks only the private key's metadata (`is_file`) and reads
  only the public half. The trigger is external damage only. Basis: SPEC_impl.md, Tensorlake account: "A key directory
  without a readable pair is reported, never regenerated". The fix is to open the private key for reading (discard the
  contents, never log them) and return the existing "incomplete or unreadable" message.
- Decision: recorded under the clearly-settled rule (`review_feedback_queue/.agents/triage.md`, "Preparing each item")
  without asking (fewest-questions triage, 2026-11-21, against main ca62397de). The maintainer can revise it.
- Completion criteria: fix the code so it matches the quoted rule, with a test where practical. Size gate: about 100
  lines of diff (code, tests and docs, not counting this ledger, the queue removal or a changelog fragment); over that,
  execution blocks and brings the item back to the maintainer. Remove the feedback file and its index entry.
- Execution: planned in `plans/queue/triage-tensorlake.md`.

## trailing-environment-history-admits-agent-origin.md

- Outcome: `fix code`.
- Assessment: record_created_history (sessions.rs:2284-2345) passes `environment_target` into history without checking
  origin. tensorlake_add.rs:347 attaches the target for every caller, agent ones included. environment_history.rs:82-99
  writes `last_container_host` and evicts old rows from the shared window. `origin` is available in
  do_create_session_with_environment (sessions.rs:2096 onward). Basis: SPEC.md:5528-5532: "Only what the user explicitly
  selects in the GUI may affect the GUI's future defaults and suggestions ... anything an agent creates do not qualify".
  Fix: drop `environment_target` from the bookkeeping acceptance when the origin is Agent.
- Decision: recorded under the clearly-settled rule (`review_feedback_queue/.agents/triage.md`, "Preparing each item")
  without asking (fewest-questions triage, 2026-11-21, against main ca62397de). The maintainer can revise it.
- Completion criteria: fix the code so it matches the quoted rule, with a test where practical. Size gate: about 100
  lines of diff (code, tests and docs, not counting this ledger, the queue removal or a changelog fragment); over that,
  execution blocks and brings the item back to the maintainer. Remove the feedback file and its index entry.
- Execution: planned in `plans/queue/triage-tensorlake.md`.

## trailing-ordinary-tensorlake-add-keeps-source-mutex-through-ssh-readiness.md

- Outcome: `fix code`.
- Assessment: tensorlake_add.rs:1428-1499 holds `source_guard` across `create_host_sandbox`. Inside it,
  tensorlake/hosts.rs:484 drops only its own (None) guard and then awaits `wait_tensorlake_ssh` (:485-497). Same root
  cause as trailing-ordinary-tensorlake-add-outer-source-lock-across-ssh-readiness.md, so one fix covers both. Basis:
  SPEC_impl.md:6644-6645: "One per-recipe async mutex excludes source selection/create through the first observed
  Running state from snapshot publication and deletion. Installation and supervisor attachment run outside that
  exclusion." Fix: pass the caller's guard into the create path, or drop it before the SSH wait. Both shapes behave the
  same.
- Decision: recorded under the clearly-settled rule (`review_feedback_queue/.agents/triage.md`, "Preparing each item")
  without asking (fewest-questions triage, 2026-11-21, against main ca62397de). The maintainer can revise it.
- Completion criteria: fix the code so it matches the quoted rule, with a test where practical. Size gate: about 100
  lines of diff (code, tests and docs, not counting this ledger, the queue removal or a changelog fragment); over that,
  execution blocks and brings the item back to the maintainer. Remove the feedback file and its index entry.
- Execution: planned in `plans/queue/triage-tensorlake.md`.

## trailing-ordinary-tensorlake-add-outer-source-lock-across-ssh-readiness.md

- Outcome: `fix code`.
- Assessment: Same code (tensorlake_add.rs:1428-1499, tensorlake/hosts.rs:484-497). Basis: SPEC_impl.md:6644-6645
  (quoted above). Fix both in one change.
- Decision: recorded under the clearly-settled rule (`review_feedback_queue/.agents/triage.md`, "Preparing each item")
  without asking (fewest-questions triage, 2026-11-21, against main ca62397de). The maintainer can revise it.
- Completion criteria: fix the code so it matches the quoted rule, with a test where practical. Size gate: about 100
  lines of diff (code, tests and docs, not counting this ledger, the queue removal or a changelog fragment); over that,
  execution blocks and brings the item back to the maintainer. Remove the feedback file and its index entry.
- Execution: planned in `plans/queue/triage-tensorlake.md`.

## trailing-provisioning-shared-environment-preparation-progress.md

- Outcome: `fix code`.
- Assessment: tensorlake_add.rs:595 sets step 0 Running. `SessionPreparation::needed` (:58-77) inserts a Running
  preparation step at index 1, and step 0 is completed only after ensure_prepared returns (:625). The GPUI pill
  (launcher/pill.rs:110-113) and host summary (hosts/progress.rs:394-398) show the first Running step. The step names
  are now provider-aware (LaunchStepNames, :423-430), so that half of the original finding is fixed. Basis:
  SPEC.md:1563-1565: "Existing launch progress names **Preparing <recipe> for sessions…**, when needed with the
  first-launch/24-hour/update explanation, **Creating a <recipe> sandbox…** ...". Fix: complete step 0 when preparation
  is found to be needed, before inserting its Running step.
- Decision: recorded under the clearly-settled rule (`review_feedback_queue/.agents/triage.md`, "Preparing each item")
  without asking (fewest-questions triage, 2026-11-21, against main ca62397de). The maintainer can revise it.
- Completion criteria: fix the code so it matches the quoted rule, with a test where practical. Size gate: about 100
  lines of diff (code, tests and docs, not counting this ledger, the queue removal or a changelog fragment); over that,
  execution blocks and brings the item back to the maintainer. Remove the feedback file and its index entry.
- Execution: planned in `plans/queue/triage-tensorlake.md`.

## ime-commit-bypasses-quick-launch-capture.md

- Outcome: `fix code`.
- Assessment: `AttachedTerminal::replace_text_in_range` calls `commit_text`
  (`crates/gui/farhelm-gui/src/terminal/attached.rs:4272-4277`), which sends unconditionally and never consults the
  armed slot. SPEC_impl.md ~8769 admits "Input-method text that arrives without a key-down is not refused yet".
  SPEC_impl.md ~1297-1300 assigns the fix: "refusing it while armed falls to the text receiver ... which can ask whether
  the slot is armed". Basis: SPEC.md ~1364 (Quick-launch sequences): "While armed, typed characters never reach the
  terminal's program." The fix is that the terminal's commit path refuses (drops) committed text while
  `key_capture_armed` is set.
- Decision: recorded under the clearly-settled rule (`review_feedback_queue/.agents/triage.md`, "Preparing each item")
  without asking (fewest-questions triage, 2026-11-21, against main ca62397de). The maintainer can revise it.
- Completion criteria: fix the code so it matches the quoted rule, with a test where practical. Size gate: about 100
  lines of diff (code, tests and docs, not counting this ledger, the queue removal or a changelog fragment); over that,
  execution blocks and brings the item back to the maintainer. Remove the feedback file and its index entry.
- Execution: planned in `plans/queue/triage-gui-misc.md`.

## checkout-status-bypasses-host-text.md

- Outcome: `fix code`.
- Assessment: `crates/gui/farhelm-gui/src/launcher/view/checkout.rs:1198` draws `Err(words)` through `words.raw()` in
  `status_line`, a plain text child. `scan_error` (:1193) is drawn the same way as a plain string. No spoofing route is
  shown, but the type rule is explicit. Basis: SPEC_impl.md "host text" (~1161-1166): "The only element that can draw it
  escapes hidden characters and isolates text direction ... its raw value, used for editing and operations." The fix is
  to draw it through `HostSentence`/the host-text element, as the neighbouring setup refusal already does (:1100).
- Decision: recorded under the clearly-settled rule (`review_feedback_queue/.agents/triage.md`, "Preparing each item")
  without asking (fewest-questions triage, 2026-11-21, against main ca62397de). The maintainer can revise it.
- Completion criteria: fix the code so it matches the quoted rule, with a test where practical. Size gate: about 100
  lines of diff (code, tests and docs, not counting this ledger, the queue removal or a changelog fragment); over that,
  execution blocks and brings the item back to the maintainer. Remove the feedback file and its index entry.
- Execution: planned in `plans/queue/triage-gui-misc.md`.

## shell-template-approval-misstates-permission.md

- Outcome: `fix code`.
- Assessment: templates/summary.rs `permission_label` (~181-183) returns "None (shell)" for any shell harness, whatever
  the permission. approvals/model.rs:806 and the templates list (summary.rs:95) use it. The CLI path can store
  harness=bash with a concrete permission (cited in the file; I did not re-trace the CLI). Basis: SPEC.md "agent
  approval cards" ~4418-4419: "template cards keep their stored permission labels and YOLO assertions without
  interpreting an omitted permission as own setting." Also ~4396-4397: "For a template write it shows the whole
  resulting template". Fix: in the shell branch, return `permission_display(permission)` when a permission is stored and
  "None (shell)" only when none is.
- Decision: recorded under the clearly-settled rule (`review_feedback_queue/.agents/triage.md`, "Preparing each item")
  without asking (fewest-questions triage, 2026-11-21, against main ca62397de). The maintainer can revise it.
- Completion criteria: fix the code so it matches the quoted rule, with a test where practical. Size gate: about 100
  lines of diff (code, tests and docs, not counting this ledger, the queue removal or a changelog fragment); over that,
  execution blocks and brings the item back to the maintainer. Remove the feedback file and its index entry.
- Execution: planned in `plans/queue/triage-gui-misc.md`.

## folder-suggestion-enter-can-submit-launcher.md

- Outcome: `fix spec+code`.
- Assessment: In the launcher, the user arrows to a folder suggestion and presses Enter; if the folder history reply
  lands in the same frame, Enter launches instead of picking the folder. In Replace with, that launch also deletes the
  source session. Verified: Confirmed on main: history reply calls `drop_folder_highlight` (launcher/view/history.rs
  ~135), then `folder_enter` finds no highlight and Enter falls through to `press` (view.rs ~829). The window runs from
  the reply being processed to the next paint, about one frame. History is read when the launcher is first drawn or the
  host changes.
- Decision: Q1, the maintainer picked "Accept these races, but guard Replace with". (2026-11-21 fewest-questions triage
  page (main ca62397de).) Maintainer's note on execution: "yes we definitely need to batch these in a much smaller set
  of plans. please try to separate them by area of code so there is a good chance of them running concurrently."
- Completion criteria: Add one spec sentence: a key or click handled before the frame showing a just-arrived background
  update is accepted, except for destructive confirmations and launches that retire a session (Replace with). Make the
  launcher's Enter safe in Replace with so it cannot launch (and retire the source session) when the folder highlight
  was dropped in the same frame. Size gate: about 100 lines of diff (code, tests and docs, not counting this ledger, the
  queue removal or a changelog fragment); over that, execution blocks and brings the item back to the maintainer. Remove
  the feedback file and its index entry.
- Execution: planned in `plans/queue/triage-gui-misc.md`.

## shell-lifecycle-approval-names-wrong-program.md

- Outcome: `fix spec+code`.
- Assessment: When an agent asks to Stop or Delete a Bash/Zsh session, the approval card says it stops "a session's
  agent" or "the agent and all terminal tabs", but approving it ends the shell and its work. Verified:
  approvals/model.rs:245 and :540–545 use fixed agent wording. `ApprovalSession` (farhelm-proto approvals.rs:84) carries
  only id, title and host, so the card cannot tell a shell from an agent. SPEC.md (~4248): "the approval card names what
  would be stopped". The Stop dialog uses "shell" for Bash/Zsh (SPEC.md ~2040).
- Decision: Q2, the maintainer picked "Yes, as a spec principle". (2026-11-21 fewest-questions triage page (main
  ca62397de).) Maintainer's note on execution: "yes we definitely need to batch these in a much smaller set of plans.
  please try to separate them by area of code so there is a good chance of them running concurrently."
- Completion criteria: Add the SPEC principle: anything that can apply to a Bash or Zsh session names the shell or is
  program-neutral, and destructive prompts say what is still alive. Word agent approval cards for Stop/Delete per launch
  kind (carry the launch kind on the card; a protocol field is accepted). Size gate: about 100 lines of diff (code,
  tests and docs, not counting this ledger, the queue removal or a changelog fragment); over that, execution blocks and
  brings the item back to the maintainer. Remove the feedback file and its index entry.
- Execution: planned in `plans/queue/triage-gui-misc.md`.

## shell-wrapper-help-names-agent.md

- Outcome: `fix code`.
- Assessment: For a Bash or Zsh launch, the wrapper prefix field's help says it runs "before the agent command".
  Verified: launcher/view.rs:1923 and :1943, templates/editor.rs:1405 and editor/rows.rs:284 all say "agent command"
  unconditionally. SPEC.md ~1058 says the wrapper also applies to shells.
- Decision: Q2, the maintainer picked "Yes, as a spec principle". (2026-11-21 fewest-questions triage page (main
  ca62397de).) Maintainer's note on execution: "yes we definitely need to batch these in a much smaller set of plans.
  please try to separate them by area of code so there is a good chance of them running concurrently."
- Completion criteria: Fix the surface (or add the test) as the Assessment describes, following the shell-naming
  principle. Size gate: about 100 lines of diff (code, tests and docs, not counting this ledger, the queue removal or a
  changelog fragment); over that, execution blocks and brings the item back to the maintainer. Remove the feedback file
  and its index entry.
- Execution: planned in `plans/queue/triage-gui-misc.md`.

## host-suspend-warning-calls-shell-agent.md

- Outcome: `fix code`.
- Assessment: In Hosts, Suspend on an environment whose only busy session is a shell asks "An agent on <host> is
  working…". Verified: hosts/lifecycle.rs:191 has a fixed "An agent on " prefix. SPEC.md ~352 itself says "when an agent
  on that host is working".
- Decision: Q2, the maintainer picked "Yes, as a spec principle". (2026-11-21 fewest-questions triage page (main
  ca62397de).) Maintainer's note on execution: "yes we definitely need to batch these in a much smaller set of plans.
  please try to separate them by area of code so there is a good chance of them running concurrently."
- Completion criteria: Fix the surface (or add the test) as the Assessment describes, following the shell-naming
  principle. Size gate: about 100 lines of diff (code, tests and docs, not counting this ledger, the queue removal or a
  changelog fragment); over that, execution blocks and brings the item back to the maintainer. Remove the feedback file
  and its index entry.
- Execution: planned in `plans/queue/triage-gui-misc.md`.

## shell-approval-command-facts-untested.md

- Outcome: `fix code`.
- Assessment: None. Approval-card tests check only that a shell launch card has no Permissions row, not that it names
  the shell and shows the command. Verified: approvals/model_tests.rs ~490-491 has only the absence assertion. The
  production branch at model.rs ~734-738 adds `shell` and `command` facts. SPEC.md ~4412: "Shell launch cards name the
  shell and show the command with no Permissions row".
- Decision: Q2, the maintainer picked "Yes, as a spec principle". (2026-11-21 fewest-questions triage page (main
  ca62397de).) Maintainer's note on execution: "yes we definitely need to batch these in a much smaller set of plans.
  please try to separate them by area of code so there is a good chance of them running concurrently."
- Completion criteria: Fix the surface (or add the test) as the Assessment describes, following the shell-naming
  principle. Size gate: about 100 lines of diff (code, tests and docs, not counting this ledger, the queue removal or a
  changelog fragment); over that, execution blocks and brings the item back to the maintainer. Remove the feedback file
  and its index entry.
- Execution: planned in `plans/queue/triage-gui-misc.md`.

## shell-stop-suspend-error-names-agent.md

- Outcome: `fix code`.
- Assessment: If Stop and suspend partly fails on a shell (or command) session, the error says "The agent stopped, but …
  the environment was not suspended". Verified: farhelm-helm sessions/sandbox.rs:584 and :591 have fixed agent wording.
  The helm does not know the program at that point.
- Decision: Q2, the maintainer picked "Yes, as a spec principle". (2026-11-21 fewest-questions triage page (main
  ca62397de).) Maintainer's note on execution: "yes we definitely need to batch these in a much smaller set of plans.
  please try to separate them by area of code so there is a good chance of them running concurrently."
- Completion criteria: Fix the surface (or add the test) as the Assessment describes, following the shell-naming
  principle. Size gate: about 100 lines of diff (code, tests and docs, not counting this ledger, the queue removal or a
  changelog fragment); over that, execution blocks and brings the item back to the maintainer. Remove the feedback file
  and its index entry.
- Execution: planned in `plans/queue/triage-tensorlake.md`.

## tensorlake-lifecycle-falls-back-to-unowned-name.md

- Outcome: `fix spec+code`.
- Assessment: If Tensorlake host setup failed because a same-named sandbox already existed, the failed host still offers
  Suspend/Resume, which then acts on that unrelated sandbox and changes its state and billing. Verified:
  tensorlake/lifecycle.rs ~132 `binding.sandbox_id.as_deref().unwrap_or(&binding.sandbox)` falls back to the name
  without an ownership check. SPEC.md (~337) states the ownership rule for removal only: "A row with neither a saved
  sandbox ID nor an outstanding create request is removed without touching a same-name sandbox."
- Decision: Q8 (fix the code), the maintainer wrote: "approve as listed for any where the fix is straight forward and
  does not involve significant additional complexity. For any where that is not true, please come back to me in the next
  update on this page." This item was judged straightforward. (2026-11-21 fewest-questions triage page (main
  ca62397de).) Maintainer's note on execution: "yes we definitely need to batch these in a much smaller set of plans.
  please try to separate them by area of code so there is a good chance of them running concurrently."
- Completion criteria: Apply the recommendation: Yes. Fix spec+code: refuse without ownership facts, and use the
  canonical ID after a permitted lookup (small). If the fix turns out not to be straightforward, or needs significant
  additional complexity, stop and bring it back to the maintainer. Size gate: about 100 lines of diff (code, tests and
  docs, not counting this ledger, the queue removal or a changelog fragment); over that, execution blocks and brings the
  item back to the maintainer. Remove the feedback file and its index entry.
- Execution: planned in `plans/queue/triage-tensorlake.md`.

## shell-switch-can-submit-hidden-conversation-id.md

- Outcome: `fix spec+code`.
- Assessment: In New, a conversation ID typed for one agent stays in the draft while you switch to a shell (where the
  field is hidden). Pressing Enter on a different agent's choice then launches immediately with that hidden ID, so the
  launch tries to continue an unintended conversation or fails. Verified: Confirmed. draft.rs keeps one `conversation`
  string across agent changes, request.rs ~520-526 includes it for any agent that supports a selector, and Enter on a
  choice applies and launches (SPEC.md ~1147-1148). SPEC.md ~1160-1168 describes a one-time selector whose label differs
  per agent, and says nothing about it surviving an agent change.
- Decision: Q8 (fix the code), the maintainer wrote: "approve as listed for any where the fix is straight forward and
  does not involve significant additional complexity. For any where that is not true, please come back to me in the next
  update on this page." This item was judged straightforward. (2026-11-21 fewest-questions triage page (main
  ca62397de).) Maintainer's note on execution: "yes we definitely need to batch these in a much smaller set of plans.
  please try to separate them by area of code so there is a good chance of them running concurrently."
- Completion criteria: Apply the recommendation: Fix code (clear the selector when the agent changes) and add a sentence
  to SPEC. If the fix turns out not to be straightforward, or needs significant additional complexity, stop and bring it
  back to the maintainer. Size gate: about 100 lines of diff (code, tests and docs, not counting this ledger, the queue
  removal or a changelog fragment); over that, execution blocks and brings the item back to the maintainer. Remove the
  feedback file and its index entry.
- Execution: planned in `plans/queue/triage-gui-misc.md`.

## successful-suspend-flashes-connecting.md

- Outcome: `fix code`.
- Assessment: After a sandbox host is confirmed Suspended, its row can briefly show Connecting before it settles back to
  Suspended. The landed tensorlake-ui-fixes plan report records seeing this flash. Verified: The mechanism is still
  present. `environment/lifecycle.rs` `perform` releases the hold without a fresh window, the actor keeps its `active`
  flag, and `manager.rs` `connect_phase` publishes `Connecting` before the status check returns Suspended. The trigger
  is ordinary (Suspend on a connected host), so it is not rare and the rare-glitch filter does not apply.
- Decision: Q8 (fix the code), the maintainer wrote: "approve as listed for any where the fix is straight forward and
  does not involve significant additional complexity. For any where that is not true, please come back to me in the next
  update on this page." This item was judged straightforward. (2026-11-21 fewest-questions triage page (main
  ca62397de).) Maintainer's note on execution: "yes we definitely need to batch these in a much smaller set of plans.
  please try to separate them by area of code so there is a good chance of them running concurrently."
- Completion criteria: Apply the recommendation: fix code, small (skip publishing Connecting for the first probe after a
  confirmed Suspend). If the fix turns out not to be straightforward, or needs significant additional complexity, stop
  and bring it back to the maintainer. Size gate: about 100 lines of diff (code, tests and docs, not counting this
  ledger, the queue removal or a changelog fragment); over that, execution blocks and brings the item back to the
  maintainer. Remove the feedback file and its index entry.
- Execution: planned in `plans/queue/triage-tensorlake.md`.

## package-repair-does-not-wait-for-dpkg-lock.md

- Outcome: `fix code`.
- Assessment: A snapshot build can fail right after boot if Ubuntu's background apt timers hold the package lock. The
  `dpkg --configure -a` repair step does not wait for the lock the way the later apt-get steps do (600 s). Verified:
  Confirmed (`tensorlake/build_script.rs` `packages`: the lock timeout applies only to apt-get, and the bare
  `dpkg --configure -a` runs first with `|| fail`). The code's own comment gives boot-time apt timers as the reason for
  the timeout, which is the same premise. Not observed on the Tensorlake image.
- Decision: Q8 (fix the code), the maintainer wrote: "approve as listed for any where the fix is straight forward and
  does not involve significant additional complexity. For any where that is not true, please come back to me in the next
  update on this page." This item was judged straightforward. (2026-11-21 fewest-questions triage page (main
  ca62397de).) Maintainer's note on execution: "yes we definitely need to batch these in a much smaller set of plans.
  please try to separate them by area of code so there is a good chance of them running concurrently."
- Completion criteria: Apply the recommendation: fix code, tiny (give the repair step the same lock wait, for example a
  bounded retry, or run the repair through apt-get with the timeout). It matches the code's existing rationale. If the
  fix turns out not to be straightforward, or needs significant additional complexity, stop and bring it back to the
  maintainer. Size gate: about 100 lines of diff (code, tests and docs, not counting this ledger, the queue removal or a
  changelog fragment); over that, execution blocks and brings the item back to the maintainer. Remove the feedback file
  and its index entry.
- Execution: planned in `plans/queue/triage-tensorlake.md`.

## trailing-agent-environment-missing-local-ssh-key-preflight.md

- Outcome: `fix code`.
- Assessment: If Farhelm's local Tensorlake SSH key is missing or unreadable, an agent can still raise an Allow/Deny
  card for a new sandbox. Allowing it then fails with "create and register Farhelm's SSH key". The GUI launcher disables
  the same choice up front. Verified: launch_environment_for_agent (agent_requests.rs:4413 onward) checks only
  `provider.places()`. Tensorlake's places (tensorlake/hosts.rs:187-192) checks only the API key. The SSH-key check
  happens after Allow, in ensure_prepared (:209-215), before any billed work. SPEC_impl.md:6272-6274: "the user is never
  asked to approve a launch that is then refused before anything starts". SPEC.md:1543-1544 disables the launcher choice
  when the key is missing. Not SETTLED because the fix has a design choice (put the check in places(), which also feeds
  the GUI, or in the agent path), and because by then a host row and run are already registered.
- Decision: Q8 (fix the code), the maintainer wrote: "approve as listed for any where the fix is straight forward and
  does not involve significant additional complexity. For any where that is not true, please come back to me in the next
  update on this page." This item was judged straightforward. (2026-11-21 fewest-questions triage page (main
  ca62397de).) Maintainer's note on execution: "yes we definitely need to batch these in a much smaller set of plans.
  please try to separate them by area of code so there is a good chance of them running concurrently."
- Completion criteria: Apply the recommendation: Yes. Fix the code: report a missing local key as a Tensorlake
  place-unavailable reason, or check it in the agent path before the card. If the fix turns out not to be
  straightforward, or needs significant additional complexity, stop and bring it back to the maintainer. Size gate:
  about 100 lines of diff (code, tests and docs, not counting this ledger, the queue removal or a changelog fragment);
  over that, execution blocks and brings the item back to the maintainer. Remove the feedback file and its index entry.
- Execution: planned in `plans/queue/triage-tensorlake.md`.

## trailing-ui-stopped-container-waiting-place-offers-suspend.md

- Outcome: `fix code`.
- Assessment: In Hosts, a stopped container whose container host is disconnected offers Suspend. Choosing it asks a
  pointless question and is then refused with "use Resume", which the menu does not offer. Verified: In the GPUI app,
  hosts/offers.rs:379-389 `lifecycle_of` falls through to Suspend for `Stopped { waiting_for_place: true }`, because
  `resumable` (:395-404) excludes it. The session list treats the same state as "changing", with no action
  (session_list/environment.rs:198-210). The spec says nothing about which action to offer here.
- Decision: Q8 (fix the code), the maintainer wrote: "approve as listed for any where the fix is straight forward and
  does not involve significant additional complexity. For any where that is not true, please come back to me in the next
  update on this page." This item was judged straightforward. (2026-11-21 fewest-questions triage page (main
  ca62397de).) Maintainer's note on execution: "yes we definitely need to batch these in a much smaller set of plans.
  please try to separate them by area of code so there is a good chance of them running concurrently."
- Completion criteria: Apply the recommendation: Yes. Fix the code: return None for that state. If the fix turns out not
  to be straightforward, or needs significant additional complexity, stop and bring it back to the maintainer. Size
  gate: about 100 lines of diff (code, tests and docs, not counting this ledger, the queue removal or a changelog
  fragment); over that, execution blocks and brings the item back to the maintainer. Remove the feedback file and its
  index entry.
- Execution: planned in `plans/queue/triage-gui-misc.md`.

## gui-command-debug-includes-secret-body.md

- Outcome: `fix code`.
- Assessment: None today. A future debug log of a GUI request could print a token or login file carried in its body.
  Verified: GUI `Command` derives `Debug` over `body: Option<Value>`
  (`crates/gui/farhelm-gui/src/operations.rs:479-484`), and so does proto `ui_channel::Command`
  (`crates/farhelm-proto/src/ui_channel.rs:80-87`). The reviewer found no code that formats one. SPEC_impl.md ~5904:
  "Secret-bearing request types have no Debug implementation." That line sits in the helm's credential section; whether
  generic envelopes count is a reading.
- Decision: Q8 (fix the code), the maintainer wrote: "approve as listed for any where the fix is straight forward and
  does not involve significant additional complexity. For any where that is not true, please come back to me in the next
  update on this page." This item was judged straightforward. (2026-11-21 fewest-questions triage page (main
  ca62397de).) Maintainer's note on execution: "yes we definitely need to batch these in a much smaller set of plans.
  please try to separate them by area of code so there is a good chance of them running concurrently."
- Completion criteria: Apply the recommendation: yes. Fix code with a hand-written Debug that omits the body (both
  types). Otherwise the item fits "Harm whose route is not established" (could-be-highest, so it needs a drop check). If
  the fix turns out not to be straightforward, or needs significant additional complexity, stop and bring it back to the
  maintainer. Size gate: about 100 lines of diff (code, tests and docs, not counting this ledger, the queue removal or a
  changelog fragment); over that, execution blocks and brings the item back to the maintainer. Remove the feedback file
  and its index entry.
- Execution: planned in `plans/queue/triage-gui-misc.md`.

## untouched-host-alias-save-restores-old-value.md

- Outcome: `fix code`.
- Assessment: With a host's alias editor open, an approved agent rename can be undone if the user presses Save without
  typing before the new name shows. This can happen while the agent's edit is still waiting for the host's locks, for
  example behind a running install. Verified: the alias save sends the held value unconditionally
  (`hosts/settings.rs:265-275`). The agent edit takes the provisioning and write locks after approval
  (`farhelm-helm/src/hosts.rs:1080-1084`), so the GUI write can land after it. SPEC.md "One GUI at a time" ~5482: "A GUI
  write must not undo a change an agent or the command line made while that view was open". Only checkout-folder and
  post-clone saves carry the loaded value. The sub-second allowance (~5500) does not cover a write queued behind locks.
- Decision: Q8 (fix the code), the maintainer wrote: "approve as listed for any where the fix is straight forward and
  does not involve significant additional complexity. For any where that is not true, please come back to me in the next
  update on this page." This item was judged straightforward. (2026-11-21 fewest-questions triage page (main
  ca62397de).) Maintainer's note on execution: "yes we definitely need to batch these in a much smaller set of plans.
  please try to separate them by area of code so there is a good chance of them running concurrently."
- Completion criteria: Apply the recommendation: fix code. Treat Save on an untouched draft as Cancel. If the fix turns
  out not to be straightforward, or needs significant additional complexity, stop and bring it back to the maintainer.
  Size gate: about 100 lines of diff (code, tests and docs, not counting this ledger, the queue removal or a changelog
  fragment); over that, execution blocks and brings the item back to the maintainer. Remove the feedback file and its
  index entry.
- Execution: planned in `plans/queue/triage-gui-misc.md`.

## launcher-stale-search-selection-fails-silently.md

- Outcome: `fix code`.
- Assessment: In the launcher, pressing Enter on a recent setup that has gone stale does nothing, and choosing a host
  that has vanished clears the search as if it worked. Neither says why. Verified: not re-traced. The reviewer reports
  it as source-confirmed. The in-flight `launcher-command-field.md` keeps today's behavior ("Taking a row ... does
  exactly what taking that recent setup, template or search row does today"), so the plan does not cover it.
- Decision: Q8 (fix the code), the maintainer wrote: "approve as listed for any where the fix is straight forward and
  does not involve significant additional complexity. For any where that is not true, please come back to me in the next
  update on this page." This item was judged straightforward. (2026-11-21 fewest-questions triage page (main
  ca62397de).) Maintainer's note on execution: "yes we definitely need to batch these in a much smaller set of plans.
  please try to separate them by area of code so there is a good chance of them running concurrently."
- Completion criteria: Apply the recommendation: fix code with the launcher's existing stale-result warning, after the
  launcher plan lands. Otherwise discard. If the fix turns out not to be straightforward, or needs significant
  additional complexity, stop and bring it back to the maintainer. Size gate: about 100 lines of diff (code, tests and
  docs, not counting this ledger, the queue removal or a changelog fragment); over that, execution blocks and brings the
  item back to the maintainer. Remove the feedback file and its index entry.
- Execution: planned in `plans/queue/triage-gui-misc.md`.

## shell-template-clears-dormant-prompt.md

- Outcome: `fix spec+code`.
- Assessment: Applying a Bash or Zsh template clears an initial prompt the user typed, while picking the same shell by
  hand keeps it, unused, in the draft. Verified: SPEC.md ~1213 says applying a template has "the same effects as making
  them by hand", and ~1223 says omitted fields keep whatever the launcher holds. SPEC_impl.md ~8910 deliberately
  specifies that templates drop "a dormant prompt when a template switches to an agent that cannot take one". This is a
  real conflict between the two specs.
- Decision: Q8 (fix the code), the maintainer wrote: "approve as listed for any where the fix is straight forward and
  does not involve significant additional complexity. For any where that is not true, please come back to me in the next
  update on this page." This item was judged straightforward. (2026-11-21 fewest-questions triage page (main
  ca62397de).) Maintainer's note on execution: "yes we definitely need to batch these in a much smaller set of plans.
  please try to separate them by area of code so there is a good chance of them running concurrently."
- Completion criteria: Apply the recommendation: fix spec+code (keep the dormant prompt as a hand switch does and remove
  the SPEC_impl exception). A request already refuses to send a prompt to an agent that cannot take one. If the fix
  turns out not to be straightforward, or needs significant additional complexity, stop and bring it back to the
  maintainer. Size gate: about 100 lines of diff (code, tests and docs, not counting this ledger, the queue removal or a
  changelog fragment); over that, execution blocks and brings the item back to the maintainer. Remove the feedback file
  and its index entry.
- Execution: planned in `plans/queue/triage-gui-misc.md`.

## shell-picker-split-by-model.md

- Outcome: `fix code`.
- Assessment: In the launcher's Agent tab, the model row sits between the agent chips and the Shells group, so Shells
  moves down whenever a model-capable agent is selected. Verified: view.rs `agent_fields` (~1812-1818) splices the model
  and catalog error at index 1, between the two `harness_row` rows. The choices.rs ~162 comment says Shells go
  "immediately after the agent chips". SPEC.md ~1055 says only "after the agent chips", so this is not clearly settled.
- Decision: Q8 (fix the code), the maintainer wrote: "approve as listed for any where the fix is straight forward and
  does not involve significant additional complexity. For any where that is not true, please come back to me in the next
  update on this page." This item was judged straightforward. (2026-11-21 fewest-questions triage page (main
  ca62397de).) Maintainer's note on execution: "yes we definitely need to batch these in a much smaller set of plans.
  please try to separate them by area of code so there is a good chance of them running concurrently."
- Completion criteria: Apply the recommendation: fix code (splice after both harness rows) If the fix turns out not to
  be straightforward, or needs significant additional complexity, stop and bring it back to the maintainer. Size gate:
  about 100 lines of diff (code, tests and docs, not counting this ledger, the queue removal or a changelog fragment);
  over that, execution blocks and brings the item back to the maintainer. Remove the feedback file and its index entry.
- Execution: planned in `plans/queue/triage-gui-misc.md`.

## shell-template-hides-incompatible-permissions.md

- Outcome: `fix code`.
- Assessment: When a template that sets a permission is switched to Bash or Zsh, the template editor shows "None
  (shell)" while still keeping the old permission, and Save is refused with no visible reason on that row. Verified:
  templates/editor/rows.rs ~501-506 returns "None (shell)" without showing the stored value, though the field stays
  removable. SPEC.md ~1269: "Incompatible fields stay visible and prevent saving until corrected or removed". The row
  stays visible, but whether its stored value must show is a reading.
- Decision: Q8 (fix the code), the maintainer wrote: "approve as listed for any where the fix is straight forward and
  does not involve significant additional complexity. For any where that is not true, please come back to me in the next
  update on this page." This item was judged straightforward. (2026-11-21 fewest-questions triage page (main
  ca62397de).) Maintainer's note on execution: "yes we definitely need to batch these in a much smaller set of plans.
  please try to separate them by area of code so there is a good chance of them running concurrently."
- Completion criteria: Apply the recommendation: fix code, together with shell-template-approval-misstates-permission.md
  (show the stored value marked incompatible) If the fix turns out not to be straightforward, or needs significant
  additional complexity, stop and bring it back to the maintainer. Size gate: about 100 lines of diff (code, tests and
  docs, not counting this ledger, the queue removal or a changelog fragment); over that, execution blocks and brings the
  item back to the maintainer. Remove the feedback file and its index entry.
- Execution: planned in `plans/queue/triage-gui-misc.md`.

## trailing-sealed-delete-comment-hold-versus-seal-record.md

- Outcome: `fix code`.
- Assessment: None. A test comment says a failed host-level Delete must not release the host's connection hold.
  Production deliberately releases it, and the seal record keeps the failure. Verified: The comment is still at
  agent_requests.rs:18299-18301. hosts.rs:1377-1381 releases the hold on provider error.
- Decision: Q12, the maintainer picked "Correct the text". (2026-11-21 fewest-questions triage page (main ca62397de).)
  Maintainer's note on execution: "yes we definitely need to batch these in a much smaller set of plans. please try to
  separate them by area of code so there is a good chance of them running concurrently."
- Completion criteria: Correct the comment, doc or spec prose so it matches the accepted current behavior, as the
  Assessment describes. No behavior change. Remove the feedback file and its index entry.
- Execution: planned in `plans/queue/triage-tensorlake.md`.

## trailing-shared-suspended-view-doc-retains-tensorlake-only-contract.md

- Outcome: `fix code`.
- Assessment: None. The protocol doc for the Suspended host state still describes only Tensorlake, although paused
  containers use it too. Verified: farhelm-proto/src/ui_channel.rs:1077-1084 still reads "A Tensorlake host whose
  sandbox Tensorlake reports suspended". The user-facing refusal is already fixed (see
  trailing-shared-suspended-refusal-retains-tensorlake-noun.md).
- Decision: Q12, the maintainer picked "Correct the text". (2026-11-21 fewest-questions triage page (main ca62397de).)
  Maintainer's note on execution: "yes we definitely need to batch these in a much smaller set of plans. please try to
  separate them by area of code so there is a good chance of them running concurrently."
- Completion criteria: Correct the comment, doc or spec prose so it matches the accepted current behavior, as the
  Assessment describes. No behavior change. Remove the feedback file and its index entry.
- Execution: planned in `plans/queue/triage-tensorlake.md`.

## browser-shard-failure-sweeps-running-siblings.md

- Outcome: `other`.
- Assessment: Maintainer test tooling only. In the Tensorlake Rust test runner, one shard failing to create its sandbox,
  or failing to save local output, makes the run's final sweep terminate sibling sandboxes that are still running,
  losing their evidence. Verified: The browser half is gone: #3401 removed the browser harness, and sandbox-tests.ts is
  now only make-template and sweep. The Rust half is still true: rust-tests.ts ~164-173 uses `Promise.all` and then
  sweeps in `finally`.
- Decision: Q14, the maintainer picked "Fix only what corrupts evidence or crashes". (2026-11-21 fewest-questions triage
  page (main ca62397de).) Maintainer's note on execution: "yes we definitely need to batch these in a much smaller set
  of plans. please try to separate them by area of code so there is a good chance of them running concurrently."
- Completion criteria: Fix it if the bug crashes a documented command, loses or misattributes test or release evidence,
  turns a real verdict into a harness failure, or lets private data into a fixture; otherwise make no change. Either way
  remove the feedback file and its index entry, and say in the execution record which way it went. Recommendation for
  reference: Fix together with browser-shard-failures-exit-successfully.md and
  worker-evidence-deleted-after-download-failure.md as one small runner PR (`allSettled` before the sweep). Narrow the
  item to the Rust runner.
- Execution: planned in `plans/queue/triage-tensorlake.md`.

## browser-shard-failures-exit-successfully.md

- Outcome: `other`.
- Assessment: Maintainer test tooling only. The Tensorlake Rust test command exits successfully even when shards failed.
  It prints "SOME SHARDS FAILED" but does not set an exit code. Verified: The browser half was removed by #3401. The
  Rust half is still true: rust-tests.ts ~170 prints the result without setting `Deno.exitCode`.
- Decision: Q14, the maintainer picked "Fix only what corrupts evidence or crashes". (2026-11-21 fewest-questions triage
  page (main ca62397de).) Maintainer's note on execution: "yes we definitely need to batch these in a much smaller set
  of plans. please try to separate them by area of code so there is a good chance of them running concurrently."
- Completion criteria: Fix it if the bug crashes a documented command, loses or misattributes test or release evidence,
  turns a real verdict into a harness failure, or lets private data into a fixture; otherwise make no change. Either way
  remove the feedback file and its index entry, and say in the execution record which way it went. Recommendation for
  reference: Fix (one line).
- Execution: planned in `plans/queue/triage-tensorlake.md`.

## worker-evidence-deleted-after-download-failure.md

- Outcome: `other`.
- Assessment: Maintainer test tooling only. If copying a failed Rust shard's evidence back fails, the runner still
  terminates the sandbox and deletes the only copy. Verified: The browser half was removed by #3401. The Rust half is
  still true: rust-tests.ts `shard()` runs `tl sbx cp ... .noThrow()` and then terminates in `finally`.
- Decision: Q14, the maintainer picked "Fix only what corrupts evidence or crashes". (2026-11-21 fewest-questions triage
  page (main ca62397de).) Maintainer's note on execution: "yes we definitely need to batch these in a much smaller set
  of plans. please try to separate them by area of code so there is a good chance of them running concurrently."
- Completion criteria: Fix it if the bug crashes a documented command, loses or misattributes test or release evidence,
  turns a real verdict into a harness failure, or lets private data into a fixture; otherwise make no change. Either way
  remove the feedback file and its index entry, and say in the execution record which way it went. Recommendation for
  reference: Fix together with the two items above, or discard.
- Execution: planned in `plans/queue/triage-tensorlake.md`.

## worker-allocation-precedes-ownership-ledger.md

- Outcome: `other`.
- Assessment: Maintainer test tooling only. A Tensorlake sandbox created by the test runner can escape cleanup (and keep
  billing) if writing the local ledger fails, or if the create call times out after the sandbox was actually made.
  Verified: Still true. sandbox-tests.ts `create()` creates first and appends to the ledger afterwards. A failed ledger
  write is a local filesystem failure (accepted under "Healthy local filesystems"); the timeout case is not.
- Decision: Q14, the maintainer picked "Fix only what corrupts evidence or crashes". (2026-11-21 fewest-questions triage
  page (main ca62397de).) Maintainer's note on execution: "yes we definitely need to batch these in a much smaller set
  of plans. please try to separate them by area of code so there is a good chance of them running concurrently."
- Completion criteria: Fix it if the bug crashes a documented command, loses or misattributes test or release evidence,
  turns a real verdict into a harness failure, or lets private data into a fixture; otherwise make no change. Either way
  remove the feedback file and its index entry, and say in the execution record which way it went. Recommendation for
  reference: Discard (the remaining case is rare, and the reserved prefix makes leftovers easy to spot).
- Execution: planned in `plans/queue/triage-tensorlake.md`.

## blocking-test-cleanup-prevents-async-listener-release.md

- Outcome: `other`.
- Assessment: None in the product. One supervisor unit test (a record naming another session's host) likely spends an
  extra 5 s in teardown and leaves its temp directory for the sweep, because the stand-in host's listener cannot be
  cancelled while synchronous cleanup blocks the single-threaded runtime. Verified: host_terminals.rs:1271-1297 declares
  `state` before `other`, and `TestHost::drop` only requests `task.abort()` (test_host.rs:104-109). The reviewer's
  executor reasoning was not re-traced in Tokio. Unmeasured.
- Decision: Q14, the maintainer picked "Fix only what corrupts evidence or crashes". (2026-11-21 fewest-questions triage
  page (main ca62397de).) Maintainer's note on execution: "yes we definitely need to batch these in a much smaller set
  of plans. please try to separate them by area of code so there is a good chance of them running concurrently."
- Completion criteria: Fix it if the bug crashes a documented command, loses or misattributes test or release evidence,
  turns a real verdict into a harness failure, or lets private data into a fixture; otherwise make no change. Either way
  remove the feedback file and its index entry, and say in the execution record which way it went. Recommendation for
  reference: Fix code: an explicit async shutdown of the stand-in that unlinks its socket before the state directory
  drops.
- Execution: planned in `plans/queue/triage-supervisor.md`.

## reboot-concurrency-early-release.md

- Outcome: `other`.
- Assessment: Maintainer tests only. Two reboot tests give back their in-process harness slot before their successor
  supervisor and terminal hosts are gone. The only effect is under explicit single-process libtest runs. Verified: Still
  present (`restart_with_resume.rs` ~L1452-1454, L1249; `session_rename.rs` L361). `harness.rs` ~L563 says the permit
  caps harnesses only within one process and never coordinates nextest.
- Decision: Q14, the maintainer picked "Fix only what corrupts evidence or crashes". (2026-11-21 fewest-questions triage
  page (main ca62397de).) Maintainer's note on execution: "yes we definitely need to batch these in a much smaller set
  of plans. please try to separate them by area of code so there is a good chance of them running concurrently."
- Completion criteria: Fix it if the bug crashes a documented command, loses or misattributes test or release evidence,
  turns a real verdict into a harness failure, or lets private data into a fixture; otherwise make no change. Either way
  remove the feedback file and its index entry, and say in the execution record which way it went. Recommendation for
  reference: Discard. It matters only for an unusual diagnostic run mode and leaks nothing.
- Execution: planned in `plans/queue/triage-supervisor.md`.

## rust-sandbox-runner-passes-empty-selection.md

- Outcome: `other`.
- Assessment: Maintainer tooling. The Tensorlake Rust test runner prints "ALL SHARDS PASSED" after running nothing when
  given `--shards 0`, a non-number, or a filter that matches nothing. Verified: Confirmed.
  `scripts/tensorlake/rust-tests.ts` ~L151 does `Number(args.shards)` with no validation, and `Array.from({length: 0})`
  followed by `every()` is true. Shards run with `--no-tests warn` and only exit codes are combined.
- Decision: Q14, the maintainer picked "Fix only what corrupts evidence or crashes". (2026-11-21 fewest-questions triage
  page (main ca62397de).) Maintainer's note on execution: "yes we definitely need to batch these in a much smaller set
  of plans. please try to separate them by area of code so there is a good chance of them running concurrently."
- Completion criteria: Fix it if the bug crashes a documented command, loses or misattributes test or release evidence,
  turns a real verdict into a harness failure, or lets private data into a fixture; otherwise make no change. Either way
  remove the feedback file and its index entry, and say in the execution record which way it went. Recommendation for
  reference: Fix the code. A false pass claim is the worst kind of tooling bug, and the guard is tiny.
- Execution: planned in `plans/queue/triage-tensorlake.md`.

## rust-sandbox-pack-ignores-cargo-target-dir.md

- Outcome: `other`.
- Assessment: Maintainer tooling. A checkout built into a shared `CARGO_TARGET_DIR` cannot be packed for the Tensorlake
  Rust tests, because the runner looks for `target/debug/farhelm-fixtures`. Verified: Confirmed (`rust-tests.ts` L58).
  AGENTS.md, "Sharing the machine with other agents", explicitly supports a shared `CARGO_TARGET_DIR`.
- Decision: Q14, the maintainer picked "Fix only what corrupts evidence or crashes". (2026-11-21 fewest-questions triage
  page (main ca62397de).) Maintainer's note on execution: "yes we definitely need to batch these in a much smaller set
  of plans. please try to separate them by area of code so there is a good chance of them running concurrently."
- Completion criteria: Fix it if the bug crashes a documented command, loses or misattributes test or release evidence,
  turns a real verdict into a harness failure, or lets private data into a fixture; otherwise make no change. Either way
  remove the feedback file and its index entry, and say in the execution record which way it went. Recommendation for
  reference: Fix the code (small). At minimum, refuse with a clear message.
- Execution: planned in `plans/queue/triage-tensorlake.md`.

## rust-sandbox-staging-misquotes-checkout-path.md

- Outcome: `other`.
- Assessment: Maintainer tooling. The Tensorlake Rust runner cannot stage a checkout whose path contains a space.
  Verified: Confirmed. The `unpack` string at `rust-tests.ts` ~L88 interpolates REPO into `sh -c` without quoting. The
  repo elsewhere quotes for paths with spaces (`harness.rs` `fixture_cmd`).
- Decision: Q14, the maintainer picked "Fix only what corrupts evidence or crashes". (2026-11-21 fewest-questions triage
  page (main ca62397de).) Maintainer's note on execution: "yes we definitely need to batch these in a much smaller set
  of plans. please try to separate them by area of code so there is a good chance of them running concurrently."
- Completion criteria: Fix it if the bug crashes a documented command, loses or misattributes test or release evidence,
  turns a real verdict into a harness failure, or lets private data into a fixture; otherwise make no change. Either way
  remove the feedback file and its index entry, and say in the execution record which way it went. Recommendation for
  reference: Fix the code (tiny), or discard if checkouts with spaces are not a supported case for this runner.
- Execution: planned in `plans/queue/triage-tensorlake.md`.

## trailing-old-server-guard-loses-directory-in-consumed-restart.md

- Outcome: `other`.
- Assessment: None (maintainer machine). If an old-terminal e2e test fails during supervisor restart, the state
  directory is deleted before the private tmux server guard runs, leaving an unreachable tmux server process behind.
  Verified: harness.rs:1094-1118 on main still moves `state` into `restart_supervisor` and calls `retire_supervisor`
  (which asserts) first. old_terminals.rs:28-32 `OldServer` holds only paths and the guard. The route is
  source-confirmed and only fires on a test failure.
- Decision: Q14, the maintainer picked "Fix only what corrupts evidence or crashes". (2026-11-21 fewest-questions triage
  page (main ca62397de).) Maintainer's note on execution: "yes we definitely need to batch these in a much smaller set
  of plans. please try to separate them by area of code so there is a good chance of them running concurrently."
- Completion criteria: Fix it if the bug crashes a documented command, loses or misattributes test or release evidence,
  turns a real verdict into a harness failure, or lets private data into a fixture; otherwise make no change. Either way
  remove the feedback file and its index entry, and say in the execution record which way it went. Recommendation for
  reference: discard (failure-path only, maintainer-only leak; the old_terminals tests are themselves transitional), or
  fix if cheap
- Execution: planned in `plans/queue/triage-supervisor.md`.

## trailing-rust-worker-trims-explicit-payload-directory.md

- Outcome: `other`.
- Assessment: None (maintainer tooling). Distributed Rust test staging fails if an operator overrides the payload
  directory with a path ending in a space or tab. Verified: scripts/tensorlake/rust-tests.ts:62 on main still `.trim()`s
  the producer output before `tar -C`. No filter matches (a retry repeats the failure).
- Decision: Q14, the maintainer picked "Fix only what corrupts evidence or crashes". (2026-11-21 fewest-questions triage
  page (main ca62397de).) Maintainer's note on execution: "yes we definitely need to batch these in a much smaller set
  of plans. please try to separate them by area of code so there is a good chance of them running concurrently."
- Completion criteria: Fix it if the bug crashes a documented command, loses or misattributes test or release evidence,
  turns a real verdict into a harness failure, or lets private data into a fixture; otherwise make no change. Either way
  remove the feedback file and its index entry, and say in the execution record which way it went. Recommendation for
  reference: discard (nobody names directories with trailing whitespace). A fix is also trivial (strip only the trailing
  newline) if preferred.
- Execution: planned in `plans/queue/triage-tensorlake.md`.

## stop-wait-test-releases-exit-before-wait-witness.md

- Outcome: `other`.
- Assessment: None in the product. The test that Stop waits for a terminal's final output before classifying a failed
  launch would also pass if Stop only checked once. Verified: host_terminals.rs:736-770: the docstring openly allows the
  record to precede Stop's probe, and the test does not witness Stop pending (confirmed). The landed plan
  stop-wait-starting-launch-witness concerned a different (shutdown) test, so it does not cover this.
- Decision: Q11, the maintainer picked "Fix only the cheap ones that guard product behavior". (2026-11-21
  fewest-questions triage page (main ca62397de).) Maintainer's note on execution: "yes we definitely need to batch these
  in a much smaller set of plans. please try to separate them by area of code so there is a good chance of them running
  concurrently."
- Completion criteria: Fix the test if the fix is a few lines (about 20 at most) and the test guards a user-facing
  behavior, including a retired test whose behavior still applies; otherwise make no change. Either way remove the
  feedback file and its index entry, and say in the execution record which way it went. Recommendation for reference:
  Fix in a batch: witness Stop pending before releasing the exit record.
- Execution: planned in `plans/queue/triage-supervisor.md`.

## foreign-host-refusal-test-cannot-observe-shutdown.md

- Outcome: `other`.
- Assessment: None in the product. The test that Delete refuses to touch another session's terminal cannot see a
  Shutdown sent to it. Verified: host_terminals.rs:1271-1297 asserts the refusal text, child alive, no input and row
  kept. The stand-in records only input bytes (per the file; test_host.rs not fully re-read).
- Decision: Q11, the maintainer picked "Fix only the cheap ones that guard product behavior". (2026-11-21
  fewest-questions triage page (main ca62397de).) Maintainer's note on execution: "yes we definitely need to batch these
  in a much smaller set of plans. please try to separate them by area of code so there is a good chance of them running
  concurrently."
- Completion criteria: Fix the test if the fix is a few lines (about 20 at most) and the test guards a user-facing
  behavior, including a retired test whose behavior still applies; otherwise make no change. Either way remove the
  feedback file and its index entry, and say in the execution record which way it went. Recommendation for reference:
  Fix in a batch: add a shutdown counter to the stand-in and assert it is zero.
- Execution: planned in `plans/queue/triage-supervisor.md`.

## seed-cancellation-test-accepts-lost-emulator.md

- Outcome: `other`.
- Assessment: None in the product. The test that an ended terminal's history stays readable after a cancelled Restart
  also passes when reading reports the history lost. Verified: Accepted from the file: `timeout(...).await.is_ok()`
  accepts `Ok(Err(EmulatorLost))`.
- Decision: Q11, the maintainer picked "Fix only the cheap ones that guard product behavior". (2026-11-21
  fewest-questions triage page (main ca62397de).) Maintainer's note on execution: "yes we definitely need to batch these
  in a much smaller set of plans. please try to separate them by area of code so there is a good chance of them running
  concurrently."
- Completion criteria: Fix the test if the fix is a few lines (about 20 at most) and the test guards a user-facing
  behavior, including a retired test whose behavior still applies; otherwise make no change. Either way remove the
  feedback file and its index entry, and say in the execution record which way it went. Recommendation for reference:
  Fix in a batch: require the inner result to be `Ok`.
- Execution: planned in `plans/queue/triage-supervisor.md`.

## cancellation-test-releases-encoder-before-abort-witness.md

- Outcome: `other`.
- Assessment: None in the product. The test of cancelling a Restart during history encoding may never actually cancel
  anything. Verified: Accepted from the file: it opens the gate right after `abort()` and never checks the JoinError.
- Decision: Q11, the maintainer picked "Fix only the cheap ones that guard product behavior". (2026-11-21
  fewest-questions triage page (main ca62397de).) Maintainer's note on execution: "yes we definitely need to batch these
  in a much smaller set of plans. please try to separate them by area of code so there is a good chance of them running
  concurrently."
- Completion criteria: Fix the test if the fix is a few lines (about 20 at most) and the test guards a user-facing
  behavior, including a retired test whose behavior still applies; otherwise make no change. Either way remove the
  feedback file and its index entry, and say in the execution record which way it went. Recommendation for reference:
  Fix in a batch: await a cancelled JoinError before opening the gate.
- Execution: planned in `plans/queue/triage-supervisor.md`.

## tab-retry-test-measures-observation-time.md

- Outcome: `other`.
- Assessment: None directly. The test for the retry delay after a failed tab close does not time the retry itself, so a
  retry that came too early could still pass. Verified: Still present (`e2e/host_tabs.rs`
  `a_failed_tab_end_is_retried_by_the_reap_after_its_backoff`): the ≥900 ms check measures from the close's reply until
  cleanup is observed, and the fault hook records only a count. A false pass needs an early retry followed by slow
  observation. Never observed.
- Decision: Q11, the maintainer picked "Fix only the cheap ones that guard product behavior". (2026-11-21
  fewest-questions triage page (main ca62397de).) Maintainer's note on execution: "yes we definitely need to batch these
  in a much smaller set of plans. please try to separate them by area of code so there is a good chance of them running
  concurrently."
- Completion criteria: Fix the test if the fix is a few lines (about 20 at most) and the test guards a user-facing
  behavior, including a retired test whose behavior still applies; otherwise make no change. Either way remove the
  feedback file and its index entry, and say in the execution record which way it went. Recommendation for reference:
  discard. A correct fix needs timestamps at the retry-scheduling boundary or a deterministic clock, which is more work
  than the gap is worth.
- Execution: planned in `plans/queue/triage-supervisor.md`.

## suspended-plan-test-erases-its-suspension-premise.md

- Outcome: `other`.
- Assessment: None. The test that a setup confirmed after a sandbox was found suspended never dials it may race:
  confirmation re-registers changed install paths, which briefly republishes Connecting before the guard reads the
  state. Verified: The test still exists as `a_plan_made_while_running_does_not_run_once_suspended` (`provisioning.rs`).
  The fixture uses root/lib and root/state while the harness admits /fixture paths, so the reconfiguration happens. I
  did not verify which state the guard sees or whether it would then probe. Not in FLAKES.md.
- Decision: Q11, the maintainer picked "Fix only the cheap ones that guard product behavior". (2026-11-21
  fewest-questions triage page (main ca62397de).) Maintainer's note on execution: "yes we definitely need to batch these
  in a much smaller set of plans. please try to separate them by area of code so there is a good chance of them running
  concurrently."
- Completion criteria: Fix the test if the fix is a few lines (about 20 at most) and the test guards a user-facing
  behavior, including a retired test whose behavior still applies; otherwise make no change. Either way remove the
  feedback file and its index entry, and say in the execution record which way it went. Recommendation for reference:
  discard unless it shows up as a flake. Otherwise fix the fixture so its paths match the planned ones.
- Execution: planned in `plans/queue/triage-tensorlake.md`.

## tunnel-fixture-assumes-simple-short-temporary-root.md

- Outcome: `other`.
- Assessment: None. Two Tensorlake transport unit tests fail when the machine's temp directory path contains `%`, `"`,
  `\` or `'`, or is too long for SSH connection sharing. Verified: The tests still exist (`transport.rs`
  `a_tensorlake_row_is_dialed_with_farhelms_key` compares raw path text against the encoded `IdentityFile`). They pass
  with normal /tmp.
- Decision: Q11, the maintainer picked "Fix only the cheap ones that guard product behavior". (2026-11-21
  fewest-questions triage page (main ca62397de).) Maintainer's note on execution: "yes we definitely need to batch these
  in a much smaller set of plans. please try to separate them by area of code so there is a good chance of them running
  concurrently."
- Completion criteria: Fix the test if the fix is a few lines (about 20 at most) and the test guards a user-facing
  behavior, including a retired test whose behavior still applies; otherwise make no change. Either way remove the
  feedback file and its index entry, and say in the execution record which way it went. Recommendation for reference:
  discard.
- Execution: planned in `plans/queue/triage-tensorlake.md`.

## redirect-regression-omits-location-header.md

- Outcome: `other`.
- Assessment: None. The test meant to catch Farhelm following a Tensorlake redirect would still pass if the no-redirect
  policy were removed, because its 307 reply has no Location. Verified: Confirmed (`tensorlake/tests.rs` scripts a 307
  with no Location). Production `http.rs` sets `Policy::none()`.
- Decision: Q11, the maintainer picked "Fix only the cheap ones that guard product behavior". (2026-11-21
  fewest-questions triage page (main ca62397de).) Maintainer's note on execution: "yes we definitely need to batch these
  in a much smaller set of plans. please try to separate them by area of code so there is a good chance of them running
  concurrently."
- Completion criteria: Fix the test if the fix is a few lines (about 20 at most) and the test guards a user-facing
  behavior, including a retired test whose behavior still applies; otherwise make no change. Either way remove the
  feedback file and its index entry, and say in the execution record which way it went. Recommendation for reference:
  fix code, tiny (add a Location to a sentinel route and assert that route is never hit). It guards a security-relevant
  policy.
- Execution: planned in `plans/queue/triage-tensorlake.md`.

## grid-text-omits-blank-columns.md

- Outcome: `other`.
- Assessment: Maintainer tests only. The e2e transcript helper glues styled runs together and drops the blank columns
  between them. "fake-agent starting" reads as "fake-agentstarting", and a trailing prompt blank disappears. Verified:
  Confirmed. `harness.rs` `grid_text` joins `run.text` and ignores `run.col`. `grid_output.rs` `trim_default_blanks`
  trims default-style spaces and drops blank runs, so the gaps really are missing from what the helper sees. The
  helper's doc says "Rows keep their trailing blanks", which is false. No current assertion was found to fail.
- Decision: Q11, the maintainer picked "Fix only the cheap ones that guard product behavior". (2026-11-21
  fewest-questions triage page (main ca62397de).) Maintainer's note on execution: "yes we definitely need to batch these
  in a much smaller set of plans. please try to separate them by area of code so there is a good chance of them running
  concurrently."
- Completion criteria: Fix the test if the fix is a few lines (about 20 at most) and the test guards a user-facing
  behavior, including a retired test whose behavior still applies; otherwise make no change. Either way remove the
  feedback file and its index entry, and say in the execution record which way it went. Recommendation for reference:
  Fix the code: rebuild the gaps from run columns and correct the doc. This is the shared root of
  grid-row-is-not-program-line and argv-grid-wrap-witness, and a coordinate-aware helper would also remove their routes.
- Execution: planned in `plans/queue/triage-supervisor.md`.

## kicked-input-test-local-filter.md

- Outcome: `other`.
- Assessment: Maintainer tests only. Three takeover tests check that a displaced viewer's keystrokes never reach the
  agent. The test client discards those keystrokes locally before sending them, so the supervisor's ownership guard is
  never exercised. Verified: Confirmed. `harness.rs` `send_keys` returns quietly when `send_grid_input` refuses, and
  `helm client.rs` ~L4142 refuses a channel that has already detached. In the lease test (`session_lifecycle.rs`
  ~L407-418) the client waits for Detached first, so the local drop always happens. The shipped supervisor guard itself
  is correct.
- Decision: Q11, the maintainer picked "Fix only the cheap ones that guard product behavior". (2026-11-21
  fewest-questions triage page (main ca62397de).) Maintainer's note on execution: "yes we definitely need to batch these
  in a much smaller set of plans. please try to separate them by area of code so there is a good chance of them running
  concurrently."
- Completion criteria: Fix the test if the fix is a few lines (about 20 at most) and the test guards a user-facing
  behavior, including a retired test whose behavior still applies; otherwise make no change. Either way remove the
  feedback file and its index entry, and say in the execution record which way it went. Recommendation for reference:
  Fix the code. The test is meant to guard a security property (the supervisor enforcing input ownership regardless of
  the client), and it no longer does. Use a raw connection as the file describes.
- Execution: planned in `plans/queue/triage-supervisor.md`.

## final-stack-drop-claim-window.md

- Outcome: `other`.
- Assessment: Maintainer tests only. The restart-gap tab test drops its supervisor and builds a new one immediately. In
  a narrow window the new one comes up read-only, and the test then fails on the wrong refusal text. Verified: Still
  present (`tab_lifecycle_edges.rs` ~L62-68 has no `retire_supervisor` barrier). The window is source-supported, but no
  flake has been observed.
- Decision: Q11, the maintainer picked "Fix only the cheap ones that guard product behavior". (2026-11-21
  fewest-questions triage page (main ca62397de).) Maintainer's note on execution: "yes we definitely need to batch these
  in a much smaller set of plans. please try to separate them by area of code so there is a good chance of them running
  concurrently."
- Completion criteria: Fix the test if the fix is a few lines (about 20 at most) and the test guards a user-facing
  behavior, including a retired test whose behavior still applies; otherwise make no change. Either way remove the
  feedback file and its index entry, and say in the execution record which way it went. Recommendation for reference:
  Fix the code: use the existing `retire_supervisor` helper. Tiny.
- Execution: planned in `plans/queue/triage-supervisor.md`.

## trailing-http-golden-omits-populated-environment.md

- Outcome: `other`.
- Assessment: None. The HTTP contract golden for the host list has no sandbox or container example, so renaming the
  `environment` field or changing its shape would not fail that test. Verified: http_contract_tests.rs:126 is the only
  `environment:` entry (None). http-contract/host-list.json has no "environment" key. The GUI decodes the same Rust
  type, which limits how much a rename could actually break.
- Decision: Q11, the maintainer picked "Fix only the cheap ones that guard product behavior". (2026-11-21
  fewest-questions triage page (main ca62397de).) Maintainer's note on execution: "yes we definitely need to batch these
  in a much smaller set of plans. please try to separate them by area of code so there is a good chance of them running
  concurrently."
- Completion criteria: Fix the test if the fix is a few lines (about 20 at most) and the test guards a user-facing
  behavior, including a retired test whose behavior still applies; otherwise make no change. Either way remove the
  feedback file and its index entry, and say in the execution record which way it went. Recommendation for reference:
  Fix the code: add one populated Tensorlake or Podman row to the golden. Discarding is also defensible, since the
  shared serde type protects the GUI.
- Execution: planned in `plans/queue/triage-tensorlake.md`.

## trailing-sealed-setup-test-overlay-hides-hold-and-run-leak.md

- Outcome: `other`.
- Assessment: None. The test that a sealed sandbox's setup is refused before it takes the host compares the public
  state, which the seal overlay masks, so it would not notice a connection hold. Verified: Partly fixed: a sibling test,
  `a_setup_refused_after_its_claim_withdraws_it_through_the_feed` (tensorlake_add_tests.rs:2198-2227), now asserts that
  no run is retained. The hold half is still unobservable (:2172-2176 compares `manager.state`).
- Decision: Q11, the maintainer picked "Fix only the cheap ones that guard product behavior". (2026-11-21
  fewest-questions triage page (main ca62397de).) Maintainer's note on execution: "yes we definitely need to batch these
  in a much smaller set of plans. please try to separate them by area of code so there is a good chance of them running
  concurrently."
- Completion criteria: Fix the test if the fix is a few lines (about 20 at most) and the test guards a user-facing
  behavior, including a retired test whose behavior still applies; otherwise make no change. Either way remove the
  feedback file and its index entry, and say in the execution record which way it went. Recommendation for reference:
  Fix the code (assert the raw hold or incarnation, or narrow the docstring). Discarding is also reasonable.
- Execution: planned in `plans/queue/triage-tensorlake.md`.

## trailing-deleted-integrated-unmarked-tab-shell-delete-oracle.md

- Outcome: `other`.
- Assessment: None. An integrated test of Delete killing an unmarked tmux tab shell and its background job was removed
  while old tmux tabs were still served. Verified: that legacy branch is gone on main (`other_pane_roots` no longer
  exists, and old terminals are ended at startup). Host-path tests cover an unmarked child, but not the shell plus
  HUP-immune job combination.
- Decision: Q11, the maintainer picked "Fix only the cheap ones that guard product behavior". (2026-11-21
  fewest-questions triage page (main ca62397de).) Maintainer's note on execution: "yes we definitely need to batch these
  in a much smaller set of plans. please try to separate them by area of code so there is a good chance of them running
  concurrently."
- Completion criteria: Fix the test if the fix is a few lines (about 20 at most) and the test guards a user-facing
  behavior, including a retired test whose behavior still applies; otherwise make no change. Either way remove the
  feedback file and its index entry, and say in the execution record which way it went. Recommendation for reference:
  discard
- Execution: planned in `plans/queue/triage-supervisor.md`.

## trailing-checkout-status-borrower-reload-keeps-origin-runtime.md

- Outcome: `other`.
- Assessment: None. A supervisor test reloads sessions while the origin host's runtime is still attached, creating two
  owners of one host. The wrapper's comment says "The tests that use it have no hosts." Verified: the stale comment is
  still at `service/core.rs:7428`, and the test is still in core.rs. No flake has been observed.
- Decision: Q11, the maintainer picked "Fix only the cheap ones that guard product behavior". (2026-11-21
  fewest-questions triage page (main ca62397de).) Maintainer's note on execution: "yes we definitely need to batch these
  in a much smaller set of plans. please try to separate them by area of code so there is a good chance of them running
  concurrently."
- Completion criteria: Fix the test if the fix is a few lines (about 20 at most) and the test guards a user-facing
  behavior, including a retired test whose behavior still applies; otherwise make no change. Either way remove the
  feedback file and its index entry, and say in the execution record which way it went. Recommendation for reference:
  discard, or batch (abort the origin runtime before reload and fix the comment)
- Execution: planned in `plans/queue/triage-supervisor.md`.

## trailing-lone-surviving-old-tab-loses-its-guard-and-stop-regression-oracles.md

- Outcome: `other`.
- Assessment: None. Tests of the Delete guard and Stop for a lone surviving old tmux tab were removed while old tmux
  terminals were still served. Verified: the record itself says this is obsolete at the tip. Old terminals are now ended
  at startup, and the tmux driver is removed.
- Decision: Q11, the maintainer picked "Fix only the cheap ones that guard product behavior". (2026-11-21
  fewest-questions triage page (main ca62397de).) Maintainer's note on execution: "yes we definitely need to batch these
  in a much smaller set of plans. please try to separate them by area of code so there is a good chance of them running
  concurrently."
- Completion criteria: Fix the test if the fix is a few lines (about 20 at most) and the test guards a user-facing
  behavior, including a retired test whose behavior still applies; otherwise make no change. Either way remove the
  feedback file and its index entry, and say in the execution record which way it went. Recommendation for reference:
  discard
- Execution: planned in `plans/queue/triage-supervisor.md`.

## trailing-failed-spec-test-accepts-an-attempted-host-start-as-never-started.md

- Outcome: `other`.
- Assessment: None. The "failed spec publish never starts a terminal" test accepts any Error and checks only an empty
  hosts directory, so an attempted host start would pass. Verified: still at `service/core.rs` around :26299-26308
  (`matches!(decoded, ControlMsg::Error { .. })`, `read_dir(...).map_or(true, ...)`).
- Decision: Q11, the maintainer picked "Fix only the cheap ones that guard product behavior". (2026-11-21
  fewest-questions triage page (main ca62397de).) Maintainer's note on execution: "yes we definitely need to batch these
  in a much smaller set of plans. please try to separate them by area of code so there is a good chance of them running
  concurrently."
- Completion criteria: Fix the test if the fix is a few lines (about 20 at most) and the test guards a user-facing
  behavior, including a retired test whose behavior still applies; otherwise make no change. Either way remove the
  feedback file and its index entry, and say in the execution record which way it went. Recommendation for reference:
  discard or batch
- Execution: planned in `plans/queue/triage-supervisor.md`.

## trailing-deleted-startup-error-artifact-readonly-oracle.md

- Outcome: `other`.
- Assessment: None directly. A supervisor that doesn't own the state directory must leave a failed launch's files alone
  at startup. The only test of that went away with a tmux fixture. Verified: every `reload_sessions(...)` call in
  core.rs and core/tests/host_terminals.rs on main passes `true` (write authority). No read-only startup reload test
  exists. The read-only check runs only through periodic reconciliation.
- Decision: Q11, the maintainer picked "Fix only the cheap ones that guard product behavior". (2026-11-21
  fewest-questions triage page (main ca62397de).) Maintainer's note on execution: "yes we definitely need to batch these
  in a much smaller set of plans. please try to separate them by area of code so there is a good chance of them running
  concurrently."
- Completion criteria: Fix the test if the fix is a few lines (about 20 at most) and the test guards a user-facing
  behavior, including a retired test whose behavior still applies; otherwise make no change. Either way remove the
  feedback file and its index entry, and say in the execution record which way it went. Recommendation for reference:
  fix code (terminal-independent reload test: false leaves files, true removes them)
- Execution: planned in `plans/queue/triage-supervisor.md`.

## trailing-omp-capture-survives-owner-exit-test-loses-exit-barrier.md

- Outcome: `other`.
- Assessment: None (test quality). The test promising that an OMP session's Resume offer survives the agent exiting
  checks the offer before confirming the agent actually exited. Verified: core.rs ~:24128-24161 on main: doc says "after
  the runtime and reporter are gone", then `kill_owner()` and an immediate offer assertion. No gone/zombie wait. The
  non-first-class harness filter does not apply (no rare trigger; consequence is test evidence).
- Decision: Q11, the maintainer picked "Fix only the cheap ones that guard product behavior". (2026-11-21
  fewest-questions triage page (main ca62397de).) Maintainer's note on execution: "yes we definitely need to batch these
  in a much smaller set of plans. please try to separate them by area of code so there is a good chance of them running
  concurrently."
- Completion criteria: Fix the test if the fix is a few lines (about 20 at most) and the test guards a user-facing
  behavior, including a retired test whose behavior still applies; otherwise make no change. Either way remove the
  feedback file and its index entry, and say in the execution record which way it went. Recommendation for reference:
  fix code (wait for both pids gone before asserting) or discard as low value; leaning fix, it is small
- Execution: planned in `plans/queue/triage-supervisor.md`.

## trailing-delete-directory-admission-test-observes-only-earlier-agent-fence.md

- Outcome: `other`.
- Assessment: None. The test that Delete of an environment's last session waits for an in-flight create (so it won't
  seal over it) watches an earlier lock, not the directory-admission wait it names. Verified: handlers.rs:12093-12123 on
  main: waits on `claimed_for_test("s1")`, plants the row, drops admission, then asserts. Unchanged. Production ordering
  is reported correct.
- Decision: Q11, the maintainer picked "Fix only the cheap ones that guard product behavior". (2026-11-21
  fewest-questions triage page (main ca62397de).) Maintainer's note on execution: "yes we definitely need to batch these
  in a much smaller set of plans. please try to separate them by area of code so there is a good chance of them running
  concurrently."
- Completion criteria: Fix the test if the fix is a few lines (about 20 at most) and the test guards a user-facing
  behavior, including a retired test whose behavior still applies; otherwise make no change. Either way remove the
  feedback file and its index entry, and say in the execution record which way it went. Recommendation for reference:
  discard unless a parked-at-admission seam is cheap. The final count still catches most early decisions.
- Execution: planned in `plans/queue/triage-supervisor.md`.

## trailing-busy-host-list-limit-negative-oracle-never-reads-client-output.md

- Outcome: `other`.
- Assessment: None. The test that a session list waiting for its own limit doesn't block typing never reads the
  connection before asserting the list hasn't answered, so an early answer would go unseen. Verified:
  handlers.rs:11261-11291 on main: `send(ListSessions)`, `type_and_see_received`, then asserts on the `host.replies`
  cache, which nothing refilled in between.
- Decision: Q11, the maintainer picked "Fix only the cheap ones that guard product behavior". (2026-11-21
  fewest-questions triage page (main ca62397de).) Maintainer's note on execution: "yes we definitely need to batch these
  in a much smaller set of plans. please try to separate them by area of code so there is a good chance of them running
  concurrently."
- Completion criteria: Fix the test if the fix is a few lines (about 20 at most) and the test guards a user-facing
  behavior, including a retired test whose behavior still applies; otherwise make no change. Either way remove the
  feedback file and its index entry, and say in the execution record which way it went. Recommendation for reference:
  fix code (drain client output up to the echo before the negative assertion). Cheap.
- Execution: planned in `plans/queue/triage-supervisor.md`.

## trailing-request-conformance-target-testhost-owner-dropped-at-preparation-return.md

- Outcome: `other`.
- Assessment: None. The terminal-control conformance test drops the stand-in host it is about to exercise before sending
  the control. Its live-terminal premise is broken, and the test can fail or pass for the wrong reason. Verified:
  handlers.rs:10870 binds `_target_host` inside the preparation future, which returns only `(message, listener)`
  (~:10956). Unchanged.
- Decision: Q11, the maintainer picked "Fix only the cheap ones that guard product behavior". (2026-11-21
  fewest-questions triage page (main ca62397de).) Maintainer's note on execution: "yes we definitely need to batch these
  in a much smaller set of plans. please try to separate them by area of code so there is a good chance of them running
  concurrently."
- Completion criteria: Fix the test if the fix is a few lines (about 20 at most) and the test guards a user-facing
  behavior, including a retired test whose behavior still applies; otherwise make no change. Either way remove the
  feedback file and its index entry, and say in the execution record which way it went. Recommendation for reference:
  fix code (return the host owner with the prepared tuple)
- Execution: planned in `plans/queue/triage-supervisor.md`.

## trailing-test-one-shot-sample-before-host-catch-up.md

- Outcome: `other`.
- Assessment: None. A ticker persistence test runs one status sample without waiting for its new terminal to catch up.
  On Linux, text and CRLF can arrive as separate events, so the sample may skip the terminal and the test fails falsely.
  Verified: ticker.rs:5206-5255 on main: `spawn_host(... "printf 'work resumed\\n'; sleep 300")`, then a single
  `sample_pass` with no caught_up wait. The route is source-established by the reviewer (Linux OPOST/ONLCR), not
  observed as a flake.
- Decision: Q11, the maintainer picked "Fix only the cheap ones that guard product behavior". (2026-11-21
  fewest-questions triage page (main ca62397de).) Maintainer's note on execution: "yes we definitely need to batch these
  in a much smaller set of plans. please try to separate them by area of code so there is a good chance of them running
  concurrently."
- Completion criteria: Fix the test if the fix is a few lines (about 20 at most) and the test guards a user-facing
  behavior, including a retired test whose behavior still applies; otherwise make no change. Either way remove the
  feedback file and its index entry, and say in the execution record which way it went. Recommendation for reference:
  fix code (bounded wait for caught_up before the pass)
- Execution: planned in `plans/queue/triage-supervisor.md`.

## trailing-migrated-seal-constraint-test-accepts-duplicate-metadata-key.md

- Outcome: `other`.
- Assessment: None. The schema test for the environment seal's allowed values would still pass if an upgraded database
  lost that constraint, because its bad INSERT also violates the primary key. Verified: store.rs:8166-8167 on main:
  `INSERT ... (0, 2)` with a generic `expect_err`, and the migration inserts id 0 unconditionally.
- Decision: Q11, the maintainer picked "Fix only the cheap ones that guard product behavior". (2026-11-21
  fewest-questions triage page (main ca62397de).) Maintainer's note on execution: "yes we definitely need to batch these
  in a much smaller set of plans. please try to separate them by area of code so there is a good chance of them running
  concurrently."
- Completion criteria: Fix the test if the fix is a few lines (about 20 at most) and the test guards a user-facing
  behavior, including a retired test whose behavior still applies; otherwise make no change. Either way remove the
  feedback file and its index entry, and say in the execution record which way it went. Recommendation for reference:
  fix code (UPDATE the existing row to sealed=2 and match the CHECK error). Tiny.
- Execution: planned in `plans/queue/triage-supervisor.md`.

## trailing-647f022-pre-reap-tab-live-only-attach-oracle-not-ported.md

- Outcome: `other`.
- Assessment: None directly. A tab whose shell exited must vanish from listings and refuse attach right away, while
  Close can still destroy it. No host-tab test checks the window before the reap. Verified: no such test in
  crates/farhelm/tests/e2e/host_tabs.rs on main (test list :282-1281). SPEC_impl.md:2027 "hidden at once" still applies.
- Decision: Q11, the maintainer picked "Fix only the cheap ones that guard product behavior". (2026-11-21
  fewest-questions triage page (main ca62397de).) Maintainer's note on execution: "yes we definitely need to batch these
  in a much smaller set of plans. please try to separate them by area of code so there is a good chance of them running
  concurrently."
- Completion criteria: Fix the test if the fix is a few lines (about 20 at most) and the test guards a user-facing
  behavior, including a retired test whose behavior still applies; otherwise make no change. Either way remove the
  feedback file and its index entry, and say in the execution record which way it went. Recommendation for reference:
  fix code (port to a host tab without auto-reap)
- Execution: planned in `plans/queue/triage-supervisor.md`.

## secret-handling-tests-omit-output-assertions.md

- Outcome: `other`.
- Assessment: None directly. Credential tests would still pass if a token or login file leaked into the painted screen,
  accessibility text, refusal text or logs. Verified: taken from the reviewer's citations (spot-checked file existence,
  not every assertion). It is a broad request to add negative assertions across about 8 tests.
- Decision: Q11, the maintainer picked "Fix only the cheap ones that guard product behavior". (2026-11-21
  fewest-questions triage page (main ca62397de).) Maintainer's note on execution: "yes we definitely need to batch these
  in a much smaller set of plans. please try to separate them by area of code so there is a good chance of them running
  concurrently."
- Completion criteria: Fix the test if the fix is a few lines (about 20 at most) and the test guards a user-facing
  behavior, including a retired test whose behavior still applies; otherwise make no change. Either way remove the
  feedback file and its index entry, and say in the execution record which way it went. Recommendation for reference:
  discard or narrow. Add frame and log negative checks only where a secret is actually typed (masked input, credentials
  editor).
- Execution: planned in `plans/queue/triage-gui-misc.md`.

## moved-on-confirmation-test-accepts-wrong-focus.md

- Outcome: `other`.
- Assessment: None directly. The macOS-rule Reset test would pass if focus sat on the same wrong control before and
  after the answer settled. Verified: `crates/gui/farhelm-gui/src/prompt_tests.rs:1972-1997` compares focus after
  settling with `moved_to` and checks only that it is "not Launch". It never asserts the dialog's own handle.
- Decision: Q11, the maintainer picked "Fix only the cheap ones that guard product behavior". (2026-11-21
  fewest-questions triage page (main ca62397de).) Maintainer's note on execution: "yes we definitely need to batch these
  in a much smaller set of plans. please try to separate them by area of code so there is a good chance of them running
  concurrently."
- Completion criteria: Fix the test if the fix is a few lines (about 20 at most) and the test guards a user-facing
  behavior, including a retired test whose behavior still applies; otherwise make no change. Either way remove the
  feedback file and its index entry, and say in the execution record which way it went. Recommendation for reference:
  fix code (one assertion).
- Execution: planned in `plans/queue/triage-gui-misc.md`.

## update-restart-failure-test-follows-status-id.md

- Outcome: `other`.
- Assessment: None directly. A failed Restart to update is said only in the update readout's hover help; the test only
  checks two element IDs are absent. Verified: `update_readout/view/tests.rs:515-520` still checks only
  `!drawn("app-update-status") && !drawn("app-update-failure")`. Speculative regression (failure drawn under a new ID).
- Decision: Q11, the maintainer picked "Fix only the cheap ones that guard product behavior". (2026-11-21
  fewest-questions triage page (main ca62397de).) Maintainer's note on execution: "yes we definitely need to batch these
  in a much smaller set of plans. please try to separate them by area of code so there is a good chance of them running
  concurrently."
- Completion criteria: Fix the test if the fix is a few lines (about 20 at most) and the test guards a user-facing
  behavior, including a retired test whose behavior still applies; otherwise make no change. Either way remove the
  feedback file and its index entry, and say in the execution record which way it went. Recommendation for reference:
  discard
- Execution: planned in `plans/queue/triage-gui-misc.md`.

## terminal-input-replays-accept-empty-fixtures.md

- Outcome: `other`.
- Assessment: None directly. Key and mouse compatibility replays would pass vacuously if their checked-in fixture tables
  were emptied. Verified: `farhelm-input-mapping/src/tests.rs` 158-196 (DOM policy) and 938-1006 (mouse) only
  `eprintln!` the count. Fixtures are `include_str!` checked-in files, so emptying needs a commit. The same file already
  pins `grid-keys.jsonl` at `assert_eq!(rows.len(), 1578)` (line 97), a direct precedent.
- Decision: Q11, the maintainer picked "Fix only the cheap ones that guard product behavior". (2026-11-21
  fewest-questions triage page (main ca62397de).) Maintainer's note on execution: "yes we definitely need to batch these
  in a much smaller set of plans. please try to separate them by area of code so there is a good chance of them running
  concurrently."
- Completion criteria: Fix the test if the fix is a few lines (about 20 at most) and the test guards a user-facing
  behavior, including a retired test whose behavior still applies; otherwise make no change. Either way remove the
  feedback file and its index entry, and say in the execution record which way it went. Recommendation for reference:
  fix code (two one-line count pins following the line-97 precedent), or discard if test-only polish is out
- Execution: planned in `plans/queue/triage-gui-misc.md`.

## wrapped-link-test-misses-second-open.md

- Outcome: `other`.
- Assessment: None directly. A terminal-link test infers that the second click opened a link from the absence of a
  refusal. Verified: `terminal/attached/tests.rs:5083-5091`; the comment explains the inference on purpose ("a refused
  click is the only outcome here that does not open"). A dropped click would also pass.
- Decision: Q11, the maintainer picked "Fix only the cheap ones that guard product behavior". (2026-11-21
  fewest-questions triage page (main ca62397de).) Maintainer's note on execution: "yes we definitely need to batch these
  in a much smaller set of plans. please try to separate them by area of code so there is a good chance of them running
  concurrently."
- Completion criteria: Fix the test if the fix is a few lines (about 20 at most) and the test guards a user-facing
  behavior, including a retired test whose behavior still applies; otherwise make no change. Either way remove the
  feedback file and its index entry, and say in the execution record which way it went. Recommendation for reference:
  discard
- Execution: planned in `plans/queue/triage-gui-misc.md`.

## inline-sign-in-tests-conflate-identifiers.md

- Outcome: `other`.
- Assessment: None directly. Add host sign-in tests give a question and its attempt the same ID, so they cannot tell
  which one Send/Cancel uses. Verified: `hosts/add/tests.rs:485-490` sets `id` and `attempt` both to `attempt`. The
  shipped form binds its own question ID.
- Decision: Q11, the maintainer picked "Fix only the cheap ones that guard product behavior". (2026-11-21
  fewest-questions triage page (main ca62397de).) Maintainer's note on execution: "yes we definitely need to batch these
  in a much smaller set of plans. please try to separate them by area of code so there is a good chance of them running
  concurrently."
- Completion criteria: Fix the test if the fix is a few lines (about 20 at most) and the test guards a user-facing
  behavior, including a retired test whose behavior still applies; otherwise make no change. Either way remove the
  feedback file and its index entry, and say in the execution record which way it went. Recommendation for reference:
  discard
- Execution: planned in `plans/queue/triage-gui-misc.md`.

## ssh-chime-test-accepts-pending-draft.md

- Outcome: `other`.
- Assessment: None. The Settings sound-switch test would pass even if a switch stayed stuck on its in-flight value
  instead of following the saved setting. Verified: The test is now the loop
  `each_sound_switch_saves_its_own_preference_field` (settings/tests.rs ~1145-1213). Each iteration pushes `!shown`, the
  same value the draft holds. `Switch::shown` (choices.rs ~443) returns the draft while saving and the stored value
  otherwise, so the final assertion cannot tell the two apart. The gap now covers all four switches, not just the last
  one.
- Decision: Q11, the maintainer picked "Fix only the cheap ones that guard product behavior". (2026-11-21
  fewest-questions triage page (main ca62397de).) Maintainer's note on execution: "yes we definitely need to batch these
  in a much smaller set of plans. please try to separate them by area of code so there is a good chance of them running
  concurrently."
- Completion criteria: Fix the test if the fix is a few lines (about 20 at most) and the test guards a user-facing
  behavior, including a retired test whose behavior still applies; otherwise make no change. Either way remove the
  feedback file and its index entry, and say in the execution record which way it went. Recommendation for reference:
  fix code (after the reply, push a different stored value and assert the switch follows it)
- Execution: planned in `plans/queue/triage-gui-misc.md`.

## antigravity-first-refused-pin-test.md

- Outcome: `other`.
- Assessment: None. Antigravity tests never check that a rejected first callback (before any verified one) leaves the
  session's process pin and identity untouched. Verified: e2e/hook_identity.rs
  `antigravity_manual_hook_captures_and_restart_passes_the_conversation` exercises rejected transcripts only after a
  verified Stop. The only pre-verification case is an unsupported event. claude_pin.rs has no Antigravity rejection
  case. SPEC_impl.md ~4505 requires rejection "before committing any pin or identity change". The production code was
  reported correct and I did not re-trace it.
- Decision: Q11, the maintainer picked "Fix only the cheap ones that guard product behavior". (2026-11-21
  fewest-questions triage page (main ca62397de).) Maintainer's note on execution: "yes we definitely need to batch these
  in a much smaller set of plans. please try to separate them by area of code so there is a good chance of them running
  concurrently."
- Completion criteria: Fix the test if the fix is a few lines (about 20 at most) and the test guards a user-facing
  behavior, including a retired test whose behavior still applies; otherwise make no change. Either way remove the
  feedback file and its index entry, and say in the execution record which way it went. Recommendation for reference:
  fix code (add a first-callback child transcript, then assert no pin or identity and that a later main Stop still
  captures). Discard is also reasonable, since Antigravity is not a first-class harness.
- Execution: planned in `plans/queue/triage-supervisor.md`.

## shell-session-e2e.md

- Outcome: `other`.
- Assessment: None directly. No end-to-end test starts a Bash or Zsh session through the normal structured launch path
  into a live terminal, so a regression between launcher and terminal could go unnoticed. Verified:
  e2e/structured_launches.rs ~92-94 makes shells `unreachable!` in the vendor fixture and skips them at ~131, ~157, ~174
  and ~309. No other runtime test uses `LaunchHarness::Bash` or `LaunchHarness::Zsh`.
- Decision: Q11, the maintainer picked "Fix only the cheap ones that guard product behavior". (2026-11-21
  fewest-questions triage page (main ca62397de).) Maintainer's note on execution: "yes we definitely need to batch these
  in a much smaller set of plans. please try to separate them by area of code so there is a good chance of them running
  concurrently."
- Completion criteria: Fix the test if the fix is a few lines (about 20 at most) and the test guards a user-facing
  behavior, including a retired test whose behavior still applies; otherwise make no change. Either way remove the
  feedback file and its index entry, and say in the execution record which way it went. Recommendation for reference:
  fix code (one e2e case per shell: launch, Running/Idle, missing-program Error)
- Execution: planned in `plans/queue/triage-supervisor.md`.

## trailing-podman-leftover-discovery-all-or-nothing-and-row-loss.md

- Outcome: `fix code`.
- Assessment: In Settings → Containers, one failing container host hides every other host's leftover containers. A
  failed Refresh also wipes the rows shown before. Verified: podman.rs `listing` returns 502 for the whole list on any
  error, by design ("a partial list would claim that the hosts not yet asked have no leftovers"). GUI
  settings/containers.rs ~255/499 replaces the rows with the error. SPEC.md (~5251) says the section follows "the
  sandbox-leftovers pattern", whose failed read "keeps earlier rows".
- Decision: returned from Q8 as not straightforward; on the follow-up page the maintainer picked "Keep earlier rows
  only" (2026-11-21 fewest-questions triage, main ca62397de). Maintainer's note on execution: "yes we definitely need to
  batch these in a much smaller set of plans. please try to separate them by area of code so there is a good chance of
  them running concurrently."
- Completion criteria: GUI only: a failed Refresh of Settings → Containers leftovers keeps the rows shown before and
  adds the error, as the sandbox-leftovers pattern SPEC cites does. A failing host still fails the whole read (no
  per-host results). Size gate: about 100 lines of diff (code, tests and docs, not counting this ledger, the queue
  removal or a changelog fragment); over that, execution blocks and brings the item back to the maintainer. Remove the
  feedback file and its index entry.
- Execution: planned in `plans/queue/triage-gui-misc.md`.

## stop-and-suspend-hides-timeout-reason.md

- Outcome: `fix code`.
- Assessment: When Stop and suspend on a sandbox session runs past 20 s, the user gets no "it carries on" message. If
  the Stop itself fails with a timeout, that failure is not shown either. Verified: the GUI deliberately drops every 504
  (`session_list/environment.rs:104-110, 667-681`). Its comment admits a supervisor's own 504 failure is dropped too:
  "the helm marks neither". SPEC.md ~1664-1665: "If the two have not finished within 20 seconds, the answer says the
  action carries on and suspends the sandbox only once the agent has stopped cleanly".
- Decision: returned from Q8 as not straightforward; on the follow-up page the maintainer picked "Mark "carries on"
  replies" (2026-11-21 fewest-questions triage, main ca62397de). Maintainer's note on execution: "yes we definitely need
  to batch these in a much smaller set of plans. please try to separate them by area of code so there is a good chance
  of them running concurrently."
- Completion criteria: Add a typed carries-on marker to the helm's bounded reply (an additive protocol field is
  accepted) so the GUI shows SPEC's "the action carries on" explanation for Stop and suspend and still shows real
  timeout failures. Size gate: about 100 lines of diff (code, tests and docs, not counting this ledger, the queue
  removal or a changelog fragment); over that, execution blocks and brings the item back to the maintainer. Remove the
  feedback file and its index entry.
- Execution: planned in `plans/queue/triage-tensorlake.md`.

## remembered-terminal-restore-keeps-unavailable-tab.md

- Outcome: `fix code`.
- Assessment: When you reopen a session whose remembered shell tab's host cannot be read until the supervisor restarts,
  the view stays on that tab, reconnecting forever, instead of falling back to the working agent terminal. Verified:
  Confirmed in the parts checked. The supervisor refuses an unreadable tab host with an Unavailable kind (handlers.rs
  ~1561-1577). The helm maps only NotFound to tab_closed (helm terminal.rs ~310-315), so the client keeps retrying.
  SPEC.md ~2299: "It shows the agent terminal if you chose Agent or if the remembered tab is gone or unavailable to
  attach." The rule is stated in so many words, but the fix has several possible shapes (a new detach code, or fallback
  logic in the GUI), so this is not SETTLED.
- Decision: returned from Q8 as not straightforward; on the follow-up page the maintainer picked "Fall back in the GUI"
  (2026-11-21 fewest-questions triage, main ca62397de). Maintainer's note on execution: "yes we definitely need to batch
  these in a much smaller set of plans. please try to separate them by area of code so there is a good chance of them
  running concurrently."
- Completion criteria: When restoring a remembered tab gets an "unavailable" refusal, the GUI shows the agent terminal,
  as SPEC (~2299) says. Size gate: about 100 lines of diff (code, tests and docs, not counting this ledger, the queue
  removal or a changelog fragment); over that, execution blocks and brings the item back to the maintainer. Remove the
  feedback file and its index entry.
- Execution: planned in `plans/queue/triage-gui-misc.md`.

## attention-cue-loses-reduced-motion-flash.md

- Outcome: `fix code`.
- Assessment: On Linux, a user with reduced motion turned on can see no attention cue at all: neither the flight nor the
  stationary flash. Verified: Farhelm reads reduced motion once via `gsettings ... enable-animations`
  (`farhelm-desktop/src/reduced_motion.rs`). GPUI Kit (`gpui_component::init`, `bootstrap.rs:84`) separately writes
  GPUI's `reduce_motion` from the XDG portal and follows it live (`gpui-base-0.9.1/src/reduce_motion.rs`). The cue
  chooses flight from Farhelm's flag (`layers/cue.rs` `activate`/`insert`), but its `cue_progress` animation jumps to
  the zero-opacity end when GPUI's flag is set. The two readings diverge on desktops where portal and GNOME settings
  differ (KDE backs the portal itself), or after a mid-run toggle. Not run on a desktop.
- Decision: returned from Q8 as not straightforward; on the follow-up page the maintainer picked "One reduced-motion
  reading everywhere" (2026-11-21 fewest-questions triage, main ca62397de). Maintainer's note on execution: "yes we
  definitely need to batch these in a much smaller set of plans. please try to separate them by area of code so there is
  a good chance of them running concurrently."
- Completion criteria: Farhelm follows the GUI toolkit's live, portal-backed reduced-motion setting everywhere and drops
  its own separate gsettings read, so the attention cue and all other motion agree (the cue shows its stationary flash
  under reduced motion). Size gate: about 100 lines of diff (code, tests and docs, not counting this ledger, the queue
  removal or a changelog fragment); over that, execution blocks and brings the item back to the maintainer. Remove the
  feedback file and its index entry.
- Execution: planned in `plans/queue/triage-gui-misc.md`.

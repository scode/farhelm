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

## credential-push-misses-retired-lifetime.md

- Outcome: `fix code`.
- Assessment: credential_delivery.rs ~506–509 still cancels on `active.wait_for(|count| *count == 0)`, which can miss a
  1→0→1 transition. Basis: SPEC_impl.md "Named credential catalog" (~5963): "Dropping the final lease cancels
  in-progress pushes before their next transport poll." Fix: latch cancellation per lifetime (a generation in the watch
  value).
- Decision: recorded under the clearly-settled rule (`review_feedback_queue/.agents/triage.md`, "Preparing each item")
  without asking (fewest-questions triage, 2026-11-21, against main ca62397de). The maintainer can revise it.
- Completion criteria: fix the code so it matches the quoted rule, with a test where practical. Size gate: about 100
  lines of diff (code, tests and docs, not counting this ledger, the queue removal or a changelog fragment); over that,
  execution blocks and brings the item back to the maintainer. Remove the feedback file and its index entry.
- Execution: planned in `plans/queue/triage-helm.md`.

## trailing-new-container-proxy-snapshot-capability-cache.md

- Outcome: `fix code`.
- Assessment: environment/podman_probe.rs `fresh` accepts a probe up to FRESH_FOR=300s; `runtime` returns a same-route
  cached result with no age check. Creation passes the cached `ready.proxy`. Basis: SPEC.md "Container sessions"
  (~5241): "Containers inherit proxy variables from the container host's systemd user manager environment at creation
  ... Later host changes affect newly created containers." Fix: read the manager's proxy variables at the creation step
  (or force a probe refresh there).
- Decision: recorded under the clearly-settled rule (`review_feedback_queue/.agents/triage.md`, "Preparing each item")
  without asking (fewest-questions triage, 2026-11-21, against main ca62397de). The maintainer can revise it.
- Completion criteria: fix the code so it matches the quoted rule, with a test where practical. Size gate: about 100
  lines of diff (code, tests and docs, not counting this ledger, the queue removal or a changelog fragment); over that,
  execution blocks and brings the item back to the maintainer. Remove the feedback file and its index entry.
- Execution: planned in `plans/queue/triage-podman.md`.

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

## valid-ssh-uri-short-label-parsing.md

- Outcome: `fix code`.
- Assessment: host_name.rs `destination_host` strips a URI port only when `is_port` (non-empty, all digits) holds, so
  the suffix stays, `IpAddr` parsing fails and the address is cut at its first dot (confirmed). The trailing-slash form
  in the file's TLDR is not detailed and is unverified. Basis: SPEC.md:287-291, "Short name": "removes `ssh://`, a user
  prefix through the last `@`, brackets, and a port only in an SSH URI ... IP addresses stay whole". Code contradicts
  this for `:ssh` and empty ports. Direct fix: in URI mode, strip any colon-free suffix after the last `:`, including an
  empty one. Caveat for the coordinator: whether a service name counts as a "port" is mildly reading-dependent;
  downgrade to DECIDE (same question as the abbreviated-IPv4 item) if you want strictness.
- Decision: recorded under the clearly-settled rule (`review_feedback_queue/.agents/triage.md`, "Preparing each item")
  without asking (fewest-questions triage, 2026-11-21, against main ca62397de). The maintainer can revise it.
- Completion criteria: fix the code so it matches the quoted rule, with a test where practical. Size gate: about 100
  lines of diff (code, tests and docs, not counting this ledger, the queue removal or a changelog fragment); over that,
  execution blocks and brings the item back to the maintainer. Remove the feedback file and its index entry.
- Execution: planned in `plans/queue/triage-helm.md`.

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

## trailing-recipe-template-management-refusal-denies-supported-agent-launch-use.md

- Outcome: `fix code`.
- Assessment: agent_requests.rs:3739 has the comment "Recipe templates stay GUI-only until agent-requested billing is
  designed" and :3744 the message. The test asserts the same text at :14961. Basis: SPEC.md:1673-1674: "Recipe templates
  stay GUI-only for listing and writing: agents cannot list, read or write them, and agent template verbs naming one are
  refused, while `farhelm agent create --template` may launch one from an ordinary host." Fix: reword the message to
  "listed and edited" (or similar), update the comment and the test assertion. The refusal itself does not change.
- Decision: recorded under the clearly-settled rule (`review_feedback_queue/.agents/triage.md`, "Preparing each item")
  without asking (fewest-questions triage, 2026-11-21, against main ca62397de). The maintainer can revise it.
- Completion criteria: fix the code so it matches the quoted rule, with a test where practical. Size gate: about 100
  lines of diff (code, tests and docs, not counting this ledger, the queue removal or a changelog fragment); over that,
  execution blocks and brings the item back to the maintainer. Remove the feedback file and its index entry.
- Execution: planned in `plans/queue/triage-helm.md`.

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

## trailing-environment-provisioning-probe-omits-stdin-watchdog.md

- Outcome: `fix code`.
- Assessment: `provisioning/backend.rs:1592-1607` (`probe_script`) and :1612-1650 (`spawn_probe`) build `internal stdio`
  without `--stdin-idle-limit-secs`. `RemoteRoute::stdin_idle_limit_secs()` (access.rs:238) already gives the per-route
  answer, and `HostAccess::stdio_argv` (access.rs:32-48) applies it elsewhere. Actual orphaning was not observed. Basis:
  SPEC_impl.md:6056-6058 "All environment stdio routes pass `--stdin-idle-limit-secs 120` unconditionally ... the
  helper's own stdin watchdog supplies the bound". Also SPEC_impl.md:6826-6828.
- Decision: recorded under the clearly-settled rule (`review_feedback_queue/.agents/triage.md`, "Preparing each item")
  without asking (fewest-questions triage, 2026-11-21, against main ca62397de). The maintainer can revise it.
- Completion criteria: fix the code so it matches the quoted rule, with a test where practical. Size gate: about 100
  lines of diff (code, tests and docs, not counting this ledger, the queue removal or a changelog fragment); over that,
  execution blocks and brings the item back to the maintainer. Remove the feedback file and its index entry.
- Execution: planned in `plans/queue/triage-helm.md`.

## trailing-podman-dial-kind-advances-before-process-marker.md

- Outcome: `fix code`.
- Assessment: manager.rs:4971-4972 advances to WaitingForHello when `row.kind.ssh_access().is_none()`, which is true for
  Podman. Yet transport.rs:803 attaches process diagnostics to every process connection. `ssh_failure_detail`
  (manager.rs:6211-6240) classifies LocalPodman and NestedSsh exits only at ConnectingToHost, so those arms are
  unreachable for real container dials. Not run. Basis: SPEC_impl.md:6064-6065 "When the fallback login fails, launch
  refusal or connection unavailability names the container host's SSH session limit."
- Decision: recorded under the clearly-settled rule (`review_feedback_queue/.agents/triage.md`, "Preparing each item")
  without asking (fewest-questions triage, 2026-11-21, against main ca62397de). The maintainer can revise it.
- Completion criteria: fix the code so it matches the quoted rule, with a test where practical. Size gate: about 100
  lines of diff (code, tests and docs, not counting this ledger, the queue removal or a changelog fragment); over that,
  execution blocks and brings the item back to the maintainer. Remove the feedback file and its index entry.
- Execution: planned in `plans/queue/triage-podman.md`.

## trailing-agent-create-cwd-help-omits-recipe-home-default.md

- Outcome: `fix code`.
- Assessment: still at `crates/farhelm/src/main.rs:320`. `--recipe` help (:340-342) already says "Folder is the new
  home". Basis: SPEC.md:4675-4676 "The destination follows the launcher's rules for a new environment: a folder is the
  new environment's home, and `--gh` clones the repository inside it."
- Decision: recorded under the clearly-settled rule (`review_feedback_queue/.agents/triage.md`, "Preparing each item")
  without asking (fewest-questions triage, 2026-11-21, against main ca62397de). The maintainer can revise it.
- Completion criteria: fix the code so it matches the quoted rule, with a test where practical. Size gate: about 100
  lines of diff (code, tests and docs, not counting this ledger, the queue removal or a changelog fragment); over that,
  execution blocks and brings the item back to the maintainer. Remove the feedback file and its index entry.
- Execution: planned in `plans/queue/triage-helm.md`.

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

## bash-stop-loses-unsaved-history.md

- Outcome: `fix code`.
- Assessment: Stop, Stop and suspend, Delete and Replace of a Bash session send SIGTERM, which interactive Bash ignores,
  then SIGKILL. The shell never exits cleanly: it sits out the grace period and loses history it would have saved on
  exit. Verified: sweep.rs ~1190–1210 and ~1879–1886 send SIGHUP only to tab-marked processes and tab scopes. The launch
  scope gets TERM then KILL, per a comment written for agents ("a launch scope holds the agent, whose graceful SIGTERM
  handling a hangup would pre-empt"). SPEC.md (~2147): "Stopping a live agent or shell ... gets its own chance to exit
  first." Zsh behavior unverified.
- Decision: Q3, the maintainer picked "Hang up shells; accept the reaping". Unsaved shell history counts as user-owned
  work for bucketing. (2026-11-21 fewest-questions triage page (main ca62397de).) Maintainer's note on execution: "yes
  we definitely need to batch these in a much smaller set of plans. please try to separate them by area of code so there
  is a good chance of them running concurrently."
- Completion criteria: End Bash/Zsh main shells on Stop, Stop and suspend, Delete and Replace with a hangup, as tab
  close does, so they exit within the grace period and save history. Decide per launch kind in the harness layer, not by
  comparing kinds at the call site. Size gate: about 100 lines of diff (code, tests and docs, not counting this ledger,
  the queue removal or a changelog fragment); over that, execution blocks and brings the item back to the maintainer.
  Remove the feedback file and its index entry.
- Execution: planned in `plans/queue/triage-supervisor.md`.

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

## trailing-archive-trash-epoch-lacks-unconditional-fleet-wake.md

- Outcome: `fix code`.
- Assessment: When a Delete archives a checkout (for example one run from the command line or by an agent while the GUI
  is open), the GUI's Trash count can stay stale until some unrelated fleet change or until Trash is opened. Verified:
  sessions.rs ~4107 bumps the Trash epoch and relies on `forget_session` to wake the fleet. manager.rs ~2711 wakes only
  if a cached row was actually removed, which an earlier listing may already have done. "One GUI at a time" removes the
  other-window framing, but CLI and agent deletes still reach this.
- Decision: Q8 (fix the code), the maintainer wrote: "approve as listed for any where the fix is straight forward and
  does not involve significant additional complexity. For any where that is not true, please come back to me in the next
  update on this page." This item was judged straightforward. (2026-11-21 fewest-questions triage page (main
  ca62397de).) Maintainer's note on execution: "yes we definitely need to batch these in a much smaller set of plans.
  please try to separate them by area of code so there is a good chance of them running concurrently."
- Completion criteria: Apply the recommendation: Fix code: always wake the fleet when `archived` is true (tiny). If the
  fix turns out not to be straightforward, or needs significant additional complexity, stop and bring it back to the
  maintainer. Size gate: about 100 lines of diff (code, tests and docs, not counting this ledger, the queue removal or a
  changelog fragment); over that, execution blocks and brings the item back to the maintainer. Remove the feedback file
  and its index entry.
- Execution: planned in `plans/queue/triage-helm.md`.

## trailing-podman-rebuild-retires-fallback-before-success.md

- Outcome: `fix code`.
- Assessment: Pressing Rebuild on a working container recipe throws away its last working images before the new build
  runs. If the rebuild fails (network or package outage), new sessions of that recipe cannot launch until a build
  succeeds. Verified: environment/podman/build.rs ~250–258 moves both images into `retired_images` at Rebuild admission,
  before `prepare_images_owned`. This deliberately forces a rebuild past the reuse shortcuts. SPEC.md (~5249):
  "Superseded Farhelm-built images are removed after successful replacement once no container uses them. Rebuild at a
  place and recipe deletion clean up that place's owned images." Tensorlake's rebuild replaces only after success
  (SPEC.md ~574).
- Decision: Q8 (fix the code), the maintainer wrote: "approve as listed for any where the fix is straight forward and
  does not involve significant additional complexity. For any where that is not true, please come back to me in the next
  update on this page." This item was judged straightforward. (2026-11-21 fewest-questions triage page (main
  ca62397de).) Maintainer's note on execution: "yes we definitely need to batch these in a much smaller set of plans.
  please try to separate them by area of code so there is a good chance of them running concurrently."
- Completion criteria: Apply the recommendation: Fix code: force the rebuild with a flag and retire the old images only
  on success. If the fix turns out not to be straightforward, or needs significant additional complexity, stop and bring
  it back to the maintainer. Size gate: about 100 lines of diff (code, tests and docs, not counting this ledger, the
  queue removal or a changelog fragment); over that, execution blocks and brings the item back to the maintainer. Remove
  the feedback file and its index entry.
- Execution: planned in `plans/queue/triage-podman.md`.

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

## gone-terminal-retained-handle-hides-recovery.md

- Outcome: `fix code`.
- Assessment: When a session's terminal host process dies but the supervisor stays up, the desktop can keep trying to
  attach to the dead terminal instead of showing the recovery card with Restart/Replace. Verified: Partly confirmed.
  `status.rs` ~341 sets `terminal_missing = entry.terminal.is_none()`, so a retained handle in `Liveness::Gone` reports
  false. `host_attach_refusal` lets Gone handles attach, and the forwarder ends with TerminalEnded. Not traced: what the
  GUI does after TerminalEnded, i.e. whether it really stays reconnecting. SPEC.md ~2304: "When a reachable host reports
  that a session has no agent terminal, the pane shows ... a recovery card". SPEC_impl.md ~8321: set "when it holds no
  agent terminal"; an unreadable (Unavailable) handle keeps the flag clear. Gone is not mentioned either way.
- Decision: Q8 (fix the code), the maintainer wrote: "approve as listed for any where the fix is straight forward and
  does not involve significant additional complexity. For any where that is not true, please come back to me in the next
  update on this page." This item was judged straightforward. (2026-11-21 fewest-questions triage page (main
  ca62397de).) Maintainer's note on execution: "yes we definitely need to batch these in a much smaller set of plans.
  please try to separate them by area of code so there is a good chance of them running concurrently."
- Completion criteria: Apply the recommendation: Fix code. Report a Gone handle as missing so the recovery card shows,
  and keep Unavailable as it is. If the fix turns out not to be straightforward, or needs significant additional
  complexity, stop and bring it back to the maintainer. Size gate: about 100 lines of diff (code, tests and docs, not
  counting this ledger, the queue removal or a changelog fragment); over that, execution blocks and brings the item back
  to the maintainer. Remove the feedback file and its index entry.
- Execution: planned in `plans/queue/triage-supervisor.md`.

## automatic-update-counts-busy-as-failure.md

- Outcome: `fix code`.
- Assessment: If you start a manual operation on an outdated host just as automatic update is planning for it, the
  automatic attempt counts as failed. The host then gets no automatic update until it reconnects or Farhelm restarts.
  Verified: Confirmed. `scan()` records the attempt before `plan_update`. A Busy error from planning or from
  `start_automatic_update` (service.rs ~1861, ~2431) takes the error path, which re-inserts the current incarnation as
  attempted. SPEC.md ~182-185: "busy hosts are reconsidered when their work finishes". The same paragraph also says "A
  planning refusal ... remembers that failed attempt", and Busy could be read as a planning refusal, so this is not
  SETTLED.
- Decision: Q8 (fix the code), the maintainer wrote: "approve as listed for any where the fix is straight forward and
  does not involve significant additional complexity. For any where that is not true, please come back to me in the next
  update on this page." This item was judged straightforward. (2026-11-21 fewest-questions triage page (main
  ca62397de).) Maintainer's note on execution: "yes we definitely need to batch these in a much smaller set of plans.
  please try to separate them by area of code so there is a good chance of them running concurrently."
- Completion criteria: Apply the recommendation: Fix code. On a typed Busy result, clear the attempt record and keep
  suppression for real failures. If the fix turns out not to be straightforward, or needs significant additional
  complexity, stop and bring it back to the maintainer. Size gate: about 100 lines of diff (code, tests and docs, not
  counting this ledger, the queue removal or a changelog fragment); over that, execution blocks and brings the item back
  to the maintainer. Remove the feedback file and its index entry.
- Execution: planned in `plans/queue/triage-helm.md`.

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

## anonymous-container-volumes-survive-removal.md

- Outcome: `fix spec+code`.
- Assessment: Deleting a container session leaves behind any anonymous volumes its image declared, which keep using disk
  on the container host. Verified: Still true for both removal paths: the legacy `rm -f --ignore` and the helm-side
  `remove_container` `rm --force` (podman.rs ~438), neither passing `--volumes`. Managed Ubuntu declares no volumes, so
  only custom images are affected. The spec says nothing about it.
- Decision: Q8 (fix the code), the maintainer wrote: "approve as listed for any where the fix is straight forward and
  does not involve significant additional complexity. For any where that is not true, please come back to me in the next
  update on this page." This item was judged straightforward. (2026-11-21 fewest-questions triage page (main
  ca62397de).) Maintainer's note on execution: "yes we definitely need to batch these in a much smaller set of plans.
  please try to separate them by area of code so there is a good chance of them running concurrently."
- Completion criteria: Apply the recommendation: Fix code (`--volumes` on owned-container removal) and add a sentence to
  SPEC. If the fix turns out not to be straightforward, or needs significant additional complexity, stop and bring it
  back to the maintainer. Size gate: about 100 lines of diff (code, tests and docs, not counting this ledger, the queue
  removal or a changelog fragment); over that, execution blocks and brings the item back to the maintainer. Remove the
  feedback file and its index entry.
- Execution: planned in `plans/queue/triage-podman.md`.

## finished-task-handle-published-after-host-purge.md

- Outcome: `fix code`.
- Assessment: None visible. When an uninstall or a failed first sandbox launch removes a host very quickly, the helm can
  keep a finished background-task handle for the removed host in memory until it restarts. Verified: Confirmed at
  service.rs:2485-2496 and tensorlake_add.rs:481-526 and 1000-1007: the task is spawned before its handle is inserted.
  `forget_host_memory` removes `tasks[host]`, so a child that finishes first leaves the parent to insert a stale handle
  afterwards. Host IDs are not reused. The cost is one small entry per occurrence.
- Decision: Q8 (fix the code), the maintainer wrote: "approve as listed for any where the fix is straight forward and
  does not involve significant additional complexity. For any where that is not true, please come back to me in the next
  update on this page." This item was judged straightforward. (2026-11-21 fewest-questions triage page (main
  ca62397de).) Maintainer's note on execution: "yes we definitely need to batch these in a much smaller set of plans.
  please try to separate them by area of code so there is a good chance of them running concurrently."
- Completion criteria: Apply the recommendation: Fix code if bundled with similar items. Hold the memory lock across
  spawn+insert at the three sites. Otherwise discard. If the fix turns out not to be straightforward, or needs
  significant additional complexity, stop and bring it back to the maintainer. Size gate: about 100 lines of diff (code,
  tests and docs, not counting this ledger, the queue removal or a changelog fragment); over that, execution blocks and
  brings the item back to the maintainer. Remove the feedback file and its index entry.
- Execution: planned in `plans/queue/triage-helm.md`.

## abbreviated-ipv4-short-label-scope-ambiguity.md

- Outcome: `fix code`.
- Assessment: A host registered as an SSH destination in IPv4 shorthand (`10.1`, which ssh reads as 10.0.0.1) shows the
  short name `10` instead of the whole address. Verified: host_name.rs:39-48 uses Rust `IpAddr` parsing, which needs
  four components, so shorthand falls through to first-label truncation (confirmed). SPEC.md:287-293 says "IP addresses
  stay whole", which leaves open whether shorthand counts as an IP address.
- Decision: Q8 (fix the code), the maintainer wrote: "approve as listed for any where the fix is straight forward and
  does not involve significant additional complexity. For any where that is not true, please come back to me in the next
  update on this page." This item was judged straightforward. (2026-11-21 fewest-questions triage page (main
  ca62397de).) Maintainer's note on execution: "yes we definitely need to batch these in a much smaller set of plans.
  please try to separate them by area of code so there is a good chance of them running concurrently."
- Completion criteria: Apply the recommendation: Yes. Fix code by keeping an all-digits-and-dots destination whole; no
  real DNS name has an all-numeric top-level label. Bundle with valid-ssh-uri-short-label-parsing.md. If the fix turns
  out not to be straightforward, or needs significant additional complexity, stop and bring it back to the maintainer.
  Size gate: about 100 lines of diff (code, tests and docs, not counting this ledger, the queue removal or a changelog
  fragment); over that, execution blocks and brings the item back to the maintainer. Remove the feedback file and its
  index entry.
- Execution: planned in `plans/queue/triage-helm.md`.

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

## mac-setup-confirmation-misses-installed-app.md

- Outcome: `fix code`.
- Assessment: Adding a remote Mac. Prepare setup, leave the confirmation dialog open, install Farhelm.app on the Mac,
  then confirm. Setup goes ahead and installs the panel's supervisor, although SPEC refuses setup on a Mac with the
  desktop app. Verified: Confirmed. `provisioning/service.rs` `revalidate` (~L2674+) re-probes for a supervisor, and
  `ProbeObservation::Absent` leads to `Execute` without repeating the app check in `backend.rs` (~L3870). SPEC.md ~L213
  refuses setup with the app installed, and SPEC_impl.md ~L7868 says consuming the id "revalidates the host and registry
  facts the plan relied on". Whether app absence counts as such a fact is a reading of the text. The 2026-10-11
  best-effort exception (SPEC.md ~L5775) covers queued or running runs, not an unconfirmed plan.
- Decision: Q8 (fix the code), the maintainer wrote: "approve as listed for any where the fix is straight forward and
  does not involve significant additional complexity. For any where that is not true, please come back to me in the next
  update on this page." This item was judged straightforward. (2026-11-21 fewest-questions triage page (main
  ca62397de).) Maintainer's note on execution: "yes we definitely need to batch these in a much smaller set of plans.
  please try to separate them by area of code so there is a good chance of them running concurrently."
- Completion criteria: Apply the recommendation: Yes for supported refusals. Fix the code: repeat the Mac app check at
  confirmation. If the fix turns out not to be straightforward, or needs significant additional complexity, stop and
  bring it back to the maintainer. Size gate: about 100 lines of diff (code, tests and docs, not counting this ledger,
  the queue removal or a changelog fragment); over that, execution blocks and brings the item back to the maintainer.
  Remove the feedback file and its index entry.
- Execution: planned in `plans/queue/triage-helm.md`.

## trailing-delete-recorded-tab-scope-kept-derived-under-unusable-manager.md

- Outcome: `fix code`.
- Assessment: Deleting a session can report cleanup finished without checking a tab's recorded cgroup scope. This
  happens when the agent itself was unscoped, a tab was scoped, and the host's systemd user manager just timed out or
  answered "none". A scrubbed tab daemon could survive. Verified: SPEC_impl.md:4688-4697 says Stop, Restart, Delete and
  tab close on "a tab whose host's tag records a scope" refuse during the probe cooldown and get one re-probe after a
  definite negative. The code differs. In teardown.rs:402-429, scoped tabs go through `ScopeUnits::extend_tab`, which
  sweep.rs:1360-1367 files as _derived_. A re-probe happens only for `entry.scope`. sweep.rs:1540-1560 skips derived
  names silently when the manager is unusable. The code says this is deliberate ("Delete historically treats tab names
  as derived ... Retain that manager policy"), so this is not SETTLED. Behavior was not reproduced.
- Decision: Q8 (fix the code), the maintainer wrote: "approve as listed for any where the fix is straight forward and
  does not involve significant additional complexity. For any where that is not true, please come back to me in the next
  update on this page." This item was judged straightforward. (2026-11-21 fewest-questions triage page (main
  ca62397de).) Maintainer's note on execution: "yes we definitely need to batch these in a much smaller set of plans.
  please try to separate them by area of code so there is a good chance of them running concurrently."
- Completion criteria: Apply the recommendation: Fix the code: in Delete, treat a tab whose tag records a scope as a
  recorded unit, and keep the spec as it is. If the fix turns out not to be straightforward, or needs significant
  additional complexity, stop and bring it back to the maintainer. Size gate: about 100 lines of diff (code, tests and
  docs, not counting this ledger, the queue removal or a changelog fragment); over that, execution blocks and brings the
  item back to the maintainer. Remove the feedback file and its index entry.
- Execution: planned in `plans/queue/triage-supervisor.md`.

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

## trailing-stopped-idle-local-arc-blocks-direct-manager-drop.md

- Outcome: `fix code`.
- Assessment: None in shipped use. A stopped container's connection actor holds a strong reference to the connection
  manager while it idles. That delays, and with two such actors can prevent, cleanup when the manager is dropped without
  an explicit shutdown. Production always shuts down explicitly. Verified: manager.rs:4397
  `let manager = self.manager.upgrade();` lives across the idle select loop at :4425-4442 when the start branch is
  skipped.
- Decision: Q8 (fix the code), the maintainer wrote: "approve as listed for any where the fix is straight forward and
  does not involve significant additional complexity. For any where that is not true, please come back to me in the next
  update on this page." This item was judged straightforward. (2026-11-21 fewest-questions triage page (main
  ca62397de).) Maintainer's note on execution: "yes we definitely need to batch these in a much smaller set of plans.
  please try to separate them by area of code so there is a good chance of them running concurrently."
- Completion criteria: Apply the recommendation: Fix the code: scope the upgrade to the gate check and upgrade again
  only for the start task. Discarding is also reasonable. If the fix turns out not to be straightforward, or needs
  significant additional complexity, stop and bring it back to the maintainer. Size gate: about 100 lines of diff (code,
  tests and docs, not counting this ledger, the queue removal or a changelog fragment); over that, execution blocks and
  brings the item back to the maintainer. Remove the feedback file and its index entry.
- Execution: planned in `plans/queue/triage-podman.md`.

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

## trailing-podman-account-observation-exec-guesses-root-home.md

- Outcome: `fix code`.
- Assessment: A custom container base that meets the documented requirements but has no `/root` directory fails to
  build. Farhelm runs the account lookup as root with `/root` as its working directory. Verified:
  `environment/podman/build.rs:65-72` runs `getent` as `root_user()`, which is hard-coded to home `/root` (:412-417).
  SPEC.md's custom-base requirements do not mention `/root`. Podman and crun behavior come from the record's upstream
  citations, not a local run.
- Decision: Q8 (fix the code), the maintainer wrote: "approve as listed for any where the fix is straight forward and
  does not involve significant additional complexity. For any where that is not true, please come back to me in the next
  update on this page." This item was judged straightforward. (2026-11-21 fewest-questions triage page (main
  ca62397de).) Maintainer's note on execution: "yes we definitely need to batch these in a much smaller set of plans.
  please try to separate them by area of code so there is a good chance of them running concurrently."
- Completion criteria: Apply the recommendation: fix code (run the lookup with a neutral workdir such as `/`). Discard
  is reasonable too: an Ubuntu/Debian image without `/root` is very unusual. If the fix turns out not to be
  straightforward, or needs significant additional complexity, stop and bring it back to the maintainer. Size gate:
  about 100 lines of diff (code, tests and docs, not counting this ledger, the queue removal or a changelog fragment);
  over that, execution blocks and brings the item back to the maintainer. Remove the feedback file and its index entry.
- Execution: planned in `plans/queue/triage-podman.md`.

## trailing-delete-held-gone-host-skips-adopted-process-confirmation.md

- Outcome: `fix code`.
- Assessment: Deleting a session could report it gone while one of its terminal hosts still runs. This happens only if
  the supervisor's view of a tab's host already says "gone" and its process search missed the live process. Verified:
  still true. `end_launch_host` (supervisor `service/core/host_launch.rs:705`) treats a runtime's `Liveness::Gone` as
  ended without signalling the adopted process. Tab close (`host_tabs.rs:1063` `end_tab_host`) deliberately checks the
  adopted process first, and its comment explains why: "the socket file can go while the host process still runs".
  Delete (`session_hosts.rs:730/738`), Restart and reclaim call `end_launch_host` directly. SPEC_impl.md ~2147-2157:
  "whenever that does not confirm the end ... the verified signal to the process the runtime adopted ... so Delete never
  reports a session gone while one of its hosts may still run". Unverified: a real producer of Gone for a live host
  (connect converts Gone to Unavailable when the process search finds the host; only a per-row read miss slips through).
- Decision: Q8 (fix the code), the maintainer wrote: "approve as listed for any where the fix is straight forward and
  does not involve significant additional complexity. For any where that is not true, please come back to me in the next
  update on this page." This item was judged straightforward. (2026-11-21 fewest-questions triage page (main
  ca62397de).) Maintainer's note on execution: "yes we definitely need to batch these in a much smaller set of plans.
  please try to separate them by area of code so there is a good chance of them running concurrently."
- Completion criteria: Apply the recommendation: fix code. Move the Gone-to-adopted-process check from `end_tab_host`
  into `end_launch_host` so every caller gets it. If the fix turns out not to be straightforward, or needs significant
  additional complexity, stop and bring it back to the maintainer. Size gate: about 100 lines of diff (code, tests and
  docs, not counting this ledger, the queue removal or a changelog fragment); over that, execution blocks and brings the
  item back to the maintainer. Remove the feedback file and its index entry.
- Execution: planned in `plans/queue/triage-supervisor.md`.

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

## tunnel-listener-stops-on-single-accept-error.md

- Outcome: `fix code`.
- Assessment: After one error accepting a connection (for example a brief file-descriptor shortage), a tunnel stops
  listening, while Farhelm keeps showing it until a later list refresh silently removes it. The user has to reopen the
  link. Verified: `crates/farhelm-helm/src/tunnels.rs:297-298` runs `let Ok((socket, _)) = accepted else { break; };`.
  Nothing notifies the feed when the loop ends. By contrast the desktop's own second-launch listener sleeps and
  continues on accept errors. How often accept fails was not measured.
- Decision: Q8 (fix the code), the maintainer wrote: "approve as listed for any where the fix is straight forward and
  does not involve significant additional complexity. For any where that is not true, please come back to me in the next
  update on this page." This item was judged straightforward. (2026-11-21 fewest-questions triage page (main
  ca62397de).) Maintainer's note on execution: "yes we definitely need to batch these in a much smaller set of plans.
  please try to separate them by area of code so there is a good chance of them running concurrently."
- Completion criteria: Apply the recommendation: fix code. Continue after transient accept errors with a short backoff.
  If the fix turns out not to be straightforward, or needs significant additional complexity, stop and bring it back to
  the maintainer. Size gate: about 100 lines of diff (code, tests and docs, not counting this ledger, the queue removal
  or a changelog fragment); over that, execution blocks and brings the item back to the maintainer. Remove the feedback
  file and its index entry.
- Execution: planned in `plans/queue/triage-helm.md`.

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

## trailing-failed-host-relaunch-warn-scope-error-definitive-rollback.md

- Outcome: `fix code`.
- Assessment: Hypothetical. After a failed Restart, if the systemd scope could not be confirmed but the portable sweep
  came back clean, Farhelm restores the old terminal. A missed descendant could then run beside a later agent. Verified:
  still at `service/core/host_restart.rs:765-782` (`ScopeKillFailure::Warn`, then `definitive`). Per the record this
  predates the review range. Nobody has shown the portable sweep missing a descendant. The independent drop check
  (2026-11-21) disagreed with filtering this: SPEC.md names ordinary programs the sweep misses, and
  host_restart.rs:765-782 sweeps with Warn and returns definitive, giving up the recorded scope.
- Decision: Q8 (fix the code), the maintainer wrote: "approve as listed for any where the fix is straight forward and
  does not involve significant additional complexity. For any where that is not true, please come back to me in the next
  update on this page." This item was judged straightforward. (2026-11-21 fewest-questions triage page (main
  ca62397de).) Maintainer's note on execution: "yes we definitely need to batch these in a much smaller set of plans.
  please try to separate them by area of code so there is a good chance of them running concurrently."
- Completion criteria: Apply the recommendation: make failed-relaunch cleanup fail, and keep the scope record, when the
  sweep cannot confirm, per SPEC.md's "Cleanup of failed launches ... follows the same rule" (~2226, 2233-2237); also
  the same Warn sweep at host_restart.rs:808 (the independent drop check's lead). If the fix turns out not to be
  straightforward, or needs significant additional complexity, stop and bring it back to the maintainer. Size gate:
  about 100 lines of diff (code, tests and docs, not counting this ledger, the queue removal or a changelog fragment);
  over that, execution blocks and brings the item back to the maintainer. Remove the feedback file and its index entry.
- Execution: planned in `plans/queue/triage-supervisor.md`.

## trailing-environment-delete-docs-overstate-admission-timeout-as-decision-bound.md

- Outcome: `fix code`.
- Assessment: None directly. An internal comment on environment Delete claims every wait before the decision is bounded,
  but the sessions lock and the seal write come after the bound. Verified: handlers.rs ~1378 still says "Every lock this
  Delete waits for before deciding is inside the bound".
- Decision: Q12, the maintainer picked "Correct the text". (2026-11-21 fewest-questions triage page (main ca62397de).)
  Maintainer's note on execution: "yes we definitely need to batch these in a much smaller set of plans. please try to
  separate them by area of code so there is a good chance of them running concurrently."
- Completion criteria: Correct the comment, doc or spec prose so it matches the accepted current behavior, as the
  Assessment describes. No behavior change. Remove the feedback file and its index entry.
- Execution: planned in `plans/queue/triage-supervisor.md`.

## host-permission-copy-quotes-retired-labels.md

- Outcome: `fix code`.
- Assessment: The helm's refusal for YOLO launches tells you to turn on "start YOLO sessions here without asking", but
  the host settings switch is now labelled "Start YOLO sessions without asking". SPEC.md quotes the old label too.
  Verified: Confirmed. yolo_guard.rs ~31 and ~163 have the old text, SPEC.md ~4455 has the old text, and the GUI label
  is in hosts/settings.rs ~377. Permission behavior is unaffected.
- Decision: Q12, the maintainer picked "Correct the text". (2026-11-21 fewest-questions triage page (main ca62397de).)
  Maintainer's note on execution: "yes we definitely need to batch these in a much smaller set of plans. please try to
  separate them by area of code so there is a good chance of them running concurrently."
- Completion criteria: Correct the comment, doc or spec prose so it matches the accepted current behavior, as the
  Assessment describes. No behavior change. Remove the feedback file and its index entry.
- Execution: planned in `plans/queue/triage-helm.md`.

## host-lock-table-contradicts-startup-span.md

- Outcome: `fix code`.
- Assessment: None. The supervisor's lock-order comment says the host-adoption lock is "Never held across a connection
  to a host", but the startup reload deliberately holds it while adopting hosts. Verified: core.rs:4617-4622 still says
  it. Line 8027 takes the lock around the reload (confirmed). The reload adopting hosts under it is taken from the file.
- Decision: Q12, the maintainer picked "Correct the text". (2026-11-21 fewest-questions triage page (main ca62397de).)
  Maintainer's note on execution: "yes we definitely need to batch these in a much smaller set of plans. please try to
  separate them by area of code so there is a good chance of them running concurrently."
- Completion criteria: Correct the comment, doc or spec prose so it matches the accepted current behavior, as the
  Assessment describes. No behavior change. Remove the feedback file and its index entry.
- Execution: planned in `plans/queue/triage-supervisor.md`.

## host-flood-stall-window-claim.md

- Outcome: `fix code`.
- Assessment: Maintainer tests only. A test's doc says it runs longer than the stall window, but it runs 8 s against a
  60 s stall timeout. Its "FLOOD-" recovery marker can also match the echo of the typed command. Verified: Confirmed.
  `terminal_tabs.rs` L1487-1557 runs 8 s, and `STALL_DETACH_TIMEOUT` is 60 s (`core.rs` L209). The doc's claim is false.
- Decision: Q12, the maintainer picked "Correct the text". (2026-11-21 fewest-questions triage page (main ca62397de).)
  Maintainer's note on execution: "yes we definitely need to batch these in a much smaller set of plans. please try to
  separate them by area of code so there is a good chance of them running concurrently."
- Completion criteria: Correct the comment, doc or spec prose so it matches the accepted current behavior, as the
  Assessment describes. No behavior change. Remove the feedback file and its index entry.
- Execution: planned in `plans/queue/triage-supervisor.md`.

## trailing-agent-launch-placement-docs-obsolete-caller-contract.md

- Outcome: `fix code`.
- Assessment: None. Internal comments wrongly say that ordinary agent creates pass no recipes, and they link to a helper
  that does not exist. Verified: agent_launch.rs:149-151 and :183 still mention `resolve_environment`, which does not
  exist (the real helper is `resolve_placed`). launcher.rs:661-662 still says "Agent callers supply no recipes".
  agent_requests.rs:2465 passes recipes through resolve_placed.
- Decision: Q12, the maintainer picked "Correct the text". (2026-11-21 fewest-questions triage page (main ca62397de).)
  Maintainer's note on execution: "yes we definitely need to batch these in a much smaller set of plans. please try to
  separate them by area of code so there is a good chance of them running concurrently."
- Completion criteria: Correct the comment, doc or spec prose so it matches the accepted current behavior, as the
  Assessment describes. No behavior change. Remove the feedback file and its index entry.
- Execution: planned in `plans/queue/triage-helm.md`.

## trailing-environment-wait-comment-promises-a-relay-ordering-the-budgets-do-not-guarantee.md

- Outcome: `fix code`.
- Assessment: None. A comment promises the "still launching" answer always reaches the agent before the relay gives up.
  The spec says this is only the normal case. Verified: The comment is still at agent_requests.rs:4555-4559.
  SPEC_impl.md says the relay's extra time is "not a guaranteed ordering" (per the reviewer; consistent with the env
  section's "normally").
- Decision: Q12, the maintainer picked "Correct the text". (2026-11-21 fewest-questions triage page (main ca62397de).)
  Maintainer's note on execution: "yes we definitely need to batch these in a much smaller set of plans. please try to
  separate them by area of code so there is a good chance of them running concurrently."
- Completion criteria: Correct the comment, doc or spec prose so it matches the accepted current behavior, as the
  Assessment describes. No behavior change. Remove the feedback file and its index entry.
- Execution: planned in `plans/queue/triage-helm.md`.

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

## trailing-create-history-current-claim-comment-contradiction.md

- Outcome: `fix code`.
- Assessment: None. A create-path comment says the remembered default skips the connection-claim recheck "by design",
  but its writer is now inside the claim-guarded transaction. Verified: The comment is still at sessions.rs:1983-1986,
  and record_created_history's guard is at :2309-2345.
- Decision: Q12, the maintainer picked "Correct the text". (2026-11-21 fewest-questions triage page (main ca62397de).)
  Maintainer's note on execution: "yes we definitely need to batch these in a much smaller set of plans. please try to
  separate them by area of code so there is a good chance of them running concurrently."
- Completion criteria: Correct the comment, doc or spec prose so it matches the accepted current behavior, as the
  Assessment describes. No behavior change. Remove the feedback file and its index entry.
- Execution: planned in `plans/queue/triage-helm.md`.

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

## trailing-dial-documentation-incorrectly-exempts-podman-from-the-provider-status-gate.md

- Outcome: `fix code`.
- Assessment: None. The dial docstring says only Tensorlake hosts wait on a provider status check before dialing.
  Containers wait on it too. Verified: manager.rs:4948-4950 still has the old wording.
- Decision: Q12, the maintainer picked "Correct the text". (2026-11-21 fewest-questions triage page (main ca62397de).)
  Maintainer's note on execution: "yes we definitely need to batch these in a much smaller set of plans. please try to
  separate them by area of code so there is a good chance of them running concurrently."
- Completion criteria: Correct the comment, doc or spec prose so it matches the accepted current behavior, as the
  Assessment describes. No behavior change. Remove the feedback file and its index entry.
- Execution: planned in `plans/queue/triage-podman.md`.

## trailing-provisioning-cleanup-test-misstates-pre-0-18-tmux-ownership.md

- Outcome: `fix code`.
- Assessment: None. A test docstring says `~/.local/bin/tmux` "is the user's own tmux, not anything Farhelm installed"
  and that earlier releases "always" used the private directory. Releases before 0.18.0 did install it beside the
  binary. Verified: The docstring is still at provisioning.rs:1055-1061. SPEC.md:205 acknowledges the pre-0.18.0
  shared-folder copy.
- Decision: Q12, the maintainer picked "Correct the text". (2026-11-21 fewest-questions triage page (main ca62397de).)
  Maintainer's note on execution: "yes we definitely need to batch these in a much smaller set of plans. please try to
  separate them by area of code so there is a good chance of them running concurrently."
- Completion criteria: Correct the comment, doc or spec prose so it matches the accepted current behavior, as the
  Assessment describes. No behavior change. Remove the feedback file and its index entry.
- Execution: planned in `plans/queue/triage-helm.md`.

## trailing-ssh-provisioning-test-private-server-coverage-docstring.md

- Outcome: `fix code`.
- Assessment: None. A test's docstring says it checks that uninstall ends "the private server", but the test only starts
  and checks terminal hosts. Verified: still at `crates/farhelm-helm/src/provisioning.rs:14215-14217`. The fixture has
  no tmux setup or check.
- Decision: Q12, the maintainer picked "Correct the text". (2026-11-21 fewest-questions triage page (main ca62397de).)
  Maintainer's note on execution: "yes we definitely need to batch these in a much smaller set of plans. please try to
  separate them by area of code so there is a good chance of them running concurrently."
- Completion criteria: Correct the comment, doc or spec prose so it matches the accepted current behavior, as the
  Assessment describes. No behavior change. Remove the feedback file and its index entry.
- Execution: planned in `plans/queue/triage-helm.md`.

## trailing-clone-protocol-docs-omit-environment-hostless-contract.md

- Outcome: `fix code`.
- Assessment: None directly. Protocol docs say a Clone without a host exists only so an old request can be refused. A
  sandbox or container clone requires the host to be omitted (SPEC.md:4676). Verified: still at
  `crates/farhelm-proto/src/lib.rs:1997-1998` and :1831-1832. The helm routes a hostless environment clone at
  agent_requests.rs:3883 / 4311.
- Decision: Q12, the maintainer picked "Correct the text". (2026-11-21 fewest-questions triage page (main ca62397de).)
  Maintainer's note on execution: "yes we definitely need to batch these in a much smaller set of plans. please try to
  separate them by area of code so there is a good chance of them running concurrently."
- Completion criteria: Correct the comment, doc or spec prose so it matches the accepted current behavior, as the
  Assessment describes. No behavior change. Remove the feedback file and its index entry.
- Execution: planned in `plans/queue/triage-helm.md`.

## trailing-interrupted-doc-defines-only-reboot-attribution-after-retirement.md

- Outcome: `fix code`.
- Assessment: None. The protocol doc for the "interrupted" status says it comes only from the reboot detector, but
  ending an old private tmux server at startup also produces it. Verified: `crates/farhelm-proto/src/lib.rs:870-872`
  still says "via the one reboot detector this system has". The same docstring (:862-864) already names the second
  route.
- Decision: Q12, the maintainer picked "Correct the text". (2026-11-21 fewest-questions triage page (main ca62397de).)
  Maintainer's note on execution: "yes we definitely need to batch these in a much smaller set of plans. please try to
  separate them by area of code so there is a good chance of them running concurrently."
- Completion criteria: Correct the comment, doc or spec prose so it matches the accepted current behavior, as the
  Assessment describes. No behavior change. Remove the feedback file and its index entry.
- Execution: planned in `plans/queue/triage-supervisor.md`.

## trailing-agent-clone-wire-docs-unconditional-host-requirement.md

- Outcome: `fix code`.
- Assessment: None directly. Same stale "Clone requires a host" protocol docs as
  `trailing-clone-protocol-docs-omit-environment-hostless-contract.md`. Verified: still at
  `crates/farhelm-proto/src/lib.rs:1831-1832` and :1997-1998.
- Decision: Q12, the maintainer picked "Correct the text". (2026-11-21 fewest-questions triage page (main ca62397de).)
  Maintainer's note on execution: "yes we definitely need to batch these in a much smaller set of plans. please try to
  separate them by area of code so there is a good chance of them running concurrently."
- Completion criteria: Correct the comment, doc or spec prose so it matches the accepted current behavior, as the
  Assessment describes. No behavior change. Remove the feedback file and its index entry.
- Execution: planned in `plans/queue/triage-helm.md`.

## trailing-launch-comment-terminal-host-inherits-tab-marker.md

- Outcome: `fix code`.
- Assessment: None. A supervisor comment says terminal hosts inherit the tab marker. Hosts start with an empty
  environment, and the marker travels only in the child's launch spec. Verified: still at
  `crates/farhelm-supervisor/src/launch.rs:113-114`.
- Decision: Q12, the maintainer picked "Correct the text". (2026-11-21 fewest-questions triage page (main ca62397de).)
  Maintainer's note on execution: "yes we definitely need to batch these in a much smaller set of plans. please try to
  separate them by area of code so there is a good chance of them running concurrently."
- Completion criteria: Correct the comment, doc or spec prose so it matches the accepted current behavior, as the
  Assessment describes. No behavior change. Remove the feedback file and its index entry.
- Execution: planned in `plans/queue/triage-supervisor.md`.

## trailing-tab-settle-gate-bare-link-out-of-scope.md

- Outcome: `fix code`.
- Assessment: None. A doc comment links to `TAB_LAUNCH_SETTLE`, which is not in scope. Verified: still at
  `service/core.rs:663`.
- Decision: Q12, the maintainer picked "Correct the text". (2026-11-21 fewest-questions triage page (main ca62397de).)
  Maintainer's note on execution: "yes we definitely need to batch these in a much smaller set of plans. please try to
  separate them by area of code so there is a good chance of them running concurrently."
- Completion criteria: Correct the comment, doc or spec prose so it matches the accepted current behavior, as the
  Assessment describes. No behavior change. Remove the feedback file and its index entry.
- Execution: planned in `plans/queue/triage-supervisor.md`.

## trailing-restart-definitive-comment-promises-no-external-effects.md

- Outcome: `fix code`.
- Assessment: None. A comment says a definitive failed Restart "changed nothing outside this process". A replacement
  program may have run briefly before cleanup. Verified: still at `service/core.rs:3249-3250`.
- Decision: Q12, the maintainer picked "Correct the text". (2026-11-21 fewest-questions triage page (main ca62397de).)
  Maintainer's note on execution: "yes we definitely need to batch these in a much smaller set of plans. please try to
  separate them by area of code so there is a good chance of them running concurrently."
- Completion criteria: Correct the comment, doc or spec prose so it matches the accepted current behavior, as the
  Assessment describes. No behavior change. Remove the feedback file and its index entry.
- Execution: planned in `plans/queue/triage-supervisor.md`.

## trailing-restart-reply-stale-tab-rediscovery-comment.md

- Outcome: `fix code`.
- Assessment: None. A comment describes tmux pane reuse and post-replacement tab discovery, both of which are gone.
  Verified: still at `service/core.rs:13197-13199`.
- Decision: Q12, the maintainer picked "Correct the text". (2026-11-21 fewest-questions triage page (main ca62397de).)
  Maintainer's note on execution: "yes we definitely need to batch these in a much smaller set of plans. please try to
  separate them by area of code so there is a good chance of them running concurrently."
- Completion criteria: Correct the comment, doc or spec prose so it matches the accepted current behavior, as the
  Assessment describes. No behavior change. Remove the feedback file and its index entry.
- Execution: planned in `plans/queue/triage-supervisor.md`.

## trailing-held-start-documentation-retains-the-retired-uncancelled-tmux-child-lifetime.md

- Outcome: `fix code`.
- Assessment: None (test-fixture comment). The comment on the held-start release guard says that cancelling ends the
  fake shell only through its iteration cap. The host start now kills the first stage on drop and enforces a deadline.
  Verified: core.rs:24404-24413 and :24430-24438 on main still say "a cancelled hold ends at the cap". Kill-on-drop
  ownership in hosts/spawn.rs not re-traced, but the reviewer's citations are consistent.
- Decision: Q12, the maintainer picked "Correct the text". (2026-11-21 fewest-questions triage page (main ca62397de).)
  Maintainer's note on execution: "yes we definitely need to batch these in a much smaller set of plans. please try to
  separate them by area of code so there is a good chance of them running concurrently."
- Completion criteria: Correct the comment, doc or spec prose so it matches the accepted current behavior, as the
  Assessment describes. No behavior change. Remove the feedback file and its index entry.
- Execution: planned in `plans/queue/triage-supervisor.md`.

## trailing-host-scan-lock-map-forbids-required-host-io.md

- Outcome: `fix code`.
- Assessment: None (internal lock-order documentation). It says the host-scan fence is never held while "a terminal call
  is acquired", yet host start and adoption run under that fence. Verified: core.rs:4616 on main still reads "Never held
  while a lifecycle claim, `attachments`, or a terminal call is acquired." The surrounding text (:4593-4614) itself
  describes host starts under the fence.
- Decision: Q12, the maintainer picked "Correct the text". (2026-11-21 fewest-questions triage page (main ca62397de).)
  Maintainer's note on execution: "yes we definitely need to batch these in a much smaller set of plans. please try to
  separate them by area of code so there is a good chance of them running concurrently."
- Completion criteria: Correct the comment, doc or spec prose so it matches the accepted current behavior, as the
  Assessment describes. No behavior change. Remove the feedback file and its index entry.
- Execution: planned in `plans/queue/triage-supervisor.md`.

## trailing-sentinel-reload-comment-denies-recorded-host-delete-fallback.md

- Outcome: `fix code`.
- Assessment: None. A reload comment says that without the in-memory terminal handle, Delete would leak the host. In
  fact Delete also finds hosts through their launch records. Verified: core.rs:6834-6841 on main still says "breaks
  `Attach` and leaks the host out from under `DeleteSession`'s teardown".
- Decision: Q12, the maintainer picked "Correct the text". (2026-11-21 fewest-questions triage page (main ca62397de).)
  Maintainer's note on execution: "yes we definitely need to batch these in a much smaller set of plans. please try to
  separate them by area of code so there is a good chance of them running concurrently."
- Completion criteria: Correct the comment, doc or spec prose so it matches the accepted current behavior, as the
  Assessment describes. No behavior change. Remove the feedback file and its index entry.
- Execution: planned in `plans/queue/triage-supervisor.md`.

## trailing-terminal-less-host-orphan-doc-asserts-no-survivors.md

- Outcome: `fix code`.
- Assessment: None. A test doc says nothing of a host-orphaned row is still running. Programs that ignored the hangup
  can outlive their host. Verified: core.rs:17889-17892 on main still says "nothing of it is running".
- Decision: Q12, the maintainer picked "Correct the text". (2026-11-21 fewest-questions triage page (main ca62397de).)
  Maintainer's note on execution: "yes we definitely need to batch these in a much smaller set of plans. please try to
  separate them by area of code so there is a good chance of them running concurrently."
- Completion criteria: Correct the comment, doc or spec prose so it matches the accepted current behavior, as the
  Assessment describes. No behavior change. Remove the feedback file and its index entry.
- Execution: planned in `plans/queue/triage-supervisor.md`.

## trailing-supervisor-clone-doorway-testdoc-universal-host-requirement.md

- Outcome: `fix code`.
- Assessment: None. A test doc says "`Clone` needs a host", but sandbox/container clones legitimately omit it (SPEC.md,
  Agents and environments). Verified: handlers.rs:7204 on main still says "`Clone` needs a host;".
- Decision: Q12, the maintainer picked "Correct the text". (2026-11-21 fewest-questions triage page (main ca62397de).)
  Maintainer's note on execution: "yes we definitely need to batch these in a much smaller set of plans. please try to
  separate them by area of code so there is a good chance of them running concurrently."
- Completion criteria: Correct the comment, doc or spec prose so it matches the accepted current behavior, as the
  Assessment describes. No behavior change. Remove the feedback file and its index entry.
- Execution: planned in `plans/queue/triage-supervisor.md`.

## trailing-stop-comment-falsely-bans-retaining-original-final-screen.md

- Outcome: `fix code`.
- Assessment: None. A Stop handler comment says SPEC requires that a full-screen program's last frame is not retained.
  SPEC says the opposite for a program killed inside full-screen mode. Verified: handlers.rs:943-946 on main: "SPEC.md
  (Terminal experience) says a full-screen program's last frame is not retained after it exits". SPEC.md:3217-3219: "one
  that does not (because it was killed, say) leaves that frame showing".
- Decision: Q12, the maintainer picked "Correct the text". (2026-11-21 fewest-questions triage page (main ca62397de).)
  Maintainer's note on execution: "yes we definitely need to batch these in a much smaller set of plans. please try to
  separate them by area of code so there is a good chance of them running concurrently."
- Completion criteria: Correct the comment, doc or spec prose so it matches the accepted current behavior, as the
  Assessment describes. No behavior change. Remove the feedback file and its index entry.
- Execution: planned in `plans/queue/triage-supervisor.md`.

## trailing-sampler-docs-promise-deleted-capture-failure-reset.md

- Outcome: `fix code`.
- Assessment: None. Sampler docs promise that a "failed capture" clears trust-prompt evidence and resets the activity
  baseline. Host screen reads can't fail, and the reset code is gone. Verified: ticker.rs:372, :413, :424 on main still
  mention failed capture. SPEC_impl.md:3507, :3540, :4054-4055 carry the same obsolete "failed capture" wording.
- Decision: Q12, the maintainer picked "Correct the text". (2026-11-21 fewest-questions triage page (main ca62397de).)
  Maintainer's note on execution: "yes we definitely need to batch these in a much smaller set of plans. please try to
  separate them by area of code so there is a good chance of them running concurrently."
- Completion criteria: Correct the comment, doc or spec prose so it matches the accepted current behavior, as the
  Assessment describes. No behavior change. Remove the feedback file and its index entry.
- Execution: planned in `plans/queue/triage-supervisor.md`.

## trailing-stopped-altscreen-test-comment-claims-inactive-buffer-erasure.md

- Outcome: `fix code`.
- Assessment: None. A test comment says leaving full-screen mode erases the frame from Farhelm's emulator. The emulator
  keeps it in its inactive buffer, and only the active screen is replayed. Verified: session_lifecycle.rs:3738 on main
  still says "the alt-screen frame is gone from the terminal's emulator".
- Decision: Q12, the maintainer picked "Correct the text". (2026-11-21 fewest-questions triage page (main ca62397de).)
  Maintainer's note on execution: "yes we definitely need to batch these in a much smaller set of plans. please try to
  separate them by area of code so there is a good chance of them running concurrently."
- Completion criteria: Correct the comment, doc or spec prose so it matches the accepted current behavior, as the
  Assessment describes. No behavior change. Remove the feedback file and its index entry.
- Execution: planned in `plans/queue/triage-supervisor.md`.

## trailing-flood-fixture-false-child-exit-terminal-lifetime-rationale.md

- Outcome: `fix code`.
- Assessment: None. The flood fixture doc says program exit "would end the terminal whose history is under test".
  Terminals outlive their program (SPEC.md, Terminal experience). Verified:
  crates/farhelm-fixtures/src/fake_agent.rs:1884 on main still says this.
- Decision: Q12, the maintainer picked "Correct the text". (2026-11-21 fewest-questions triage page (main ca62397de).)
  Maintainer's note on execution: "yes we definitely need to batch these in a much smaller set of plans. please try to
  separate them by area of code so there is a good chance of them running concurrently."
- Completion criteria: Correct the comment, doc or spec prose so it matches the accepted current behavior, as the
  Assessment describes. No behavior change. Remove the feedback file and its index entry.
- Execution: planned in `plans/queue/triage-supervisor.md`.

## trailing-outcome-backfill-comment-falsely-promises-live-probe-after-legacy-retirement.md

- Outcome: `fix code`.
- Assessment: None. A migration comment says the first reload reconciles each backfilled row against its live terminal.
  Those rows are old-kind and are now retired without any probe. Verified: store.rs:1858-1860 on main still says this.
- Decision: Q12, the maintainer picked "Correct the text". (2026-11-21 fewest-questions triage page (main ca62397de).)
  Maintainer's note on execution: "yes we definitely need to batch these in a much smaller set of plans. please try to
  separate them by area of code so there is a good chance of them running concurrently."
- Completion criteria: Correct the comment, doc or spec prose so it matches the accepted current behavior, as the
  Assessment describes. No behavior change. Remove the feedback file and its index entry.
- Execution: planned in `plans/queue/triage-supervisor.md`.

## trailing-70bc66a-host-size-wait-comment-denies-public-status-size.md

- Outcome: `fix code`.
- Assessment: None. A test helper says terminal size is not a status field, so it polls. The status does carry size and
  notifies on resize. Verified: crates/farhelm/tests/e2e/terminal_tabs.rs:1394 on main still says "not a status field of
  its own".
- Decision: Q12, the maintainer picked "Correct the text". (2026-11-21 fewest-questions triage page (main ca62397de).)
  Maintainer's note on execution: "yes we definitely need to batch these in a much smaller set of plans. please try to
  separate them by area of code so there is a good chance of them running concurrently."
- Completion criteria: Correct the comment, doc or spec prose so it matches the accepted current behavior, as the
  Assessment describes. No behavior change. Remove the feedback file and its index entry.
- Execution: planned in `plans/queue/triage-supervisor.md`.

## trailing-terminal-tabs-shell-harness-docs-retain-deleted-conformance-caller.md

- Outcome: `fix code`.
- Assessment: None. A test helper's doc points at a conformance battery that was deleted. Its only caller is now the
  tab-close SIGTERM test. Verified: crates/farhelm/tests/e2e/terminal_tabs.rs:28-32 on main still says "so the
  conformance tests can drive BOTH terminals identically".
- Decision: Q12, the maintainer picked "Correct the text". (2026-11-21 fewest-questions triage page (main ca62397de).)
  Maintainer's note on execution: "yes we definitely need to batch these in a much smaller set of plans. please try to
  separate them by area of code so there is a good chance of them running concurrently."
- Completion criteria: Correct the comment, doc or spec prose so it matches the accepted current behavior, as the
  Assessment describes. No behavior change. Remove the feedback file and its index entry.
- Execution: planned in `plans/queue/triage-supervisor.md`.

## trailing-terminal-tabs-shared-gate-docs-retain-removed-local-hook-installations.md

- Outcome: `fix code`.
- Assessment: None. The shared gate helper's doc says "The two hooks this file installs". None are installed in that
  file any more; the only user is in host_tabs. Verified: crates/farhelm/tests/e2e/terminal_tabs.rs:251, :257 on main.
- Decision: Q12, the maintainer picked "Correct the text". (2026-11-21 fewest-questions triage page (main ca62397de).)
  Maintainer's note on execution: "yes we definitely need to batch these in a much smaller set of plans. please try to
  separate them by area of code so there is a good chance of them running concurrently."
- Completion criteria: Correct the comment, doc or spec prose so it matches the accepted current behavior, as the
  Assessment describes. No behavior change. Remove the feedback file and its index entry.
- Execution: planned in `plans/queue/triage-supervisor.md`.

## trailing-connection-writer-comment-points-to-deleted-outputstream-checkpoint.md

- Outcome: `fix code`.
- Assessment: None. A maintainer tracing lost terminal output is pointed to a diagnostic checkpoint that no longer
  exists. Verified: still present at `crates/farhelm-supervisor/src/service/connection.rs:990` ("see the first one in
  `OutputStream::next_output`"). No `OutputStream`/`next_output` remains anywhere in crates.
- Decision: Q12, the maintainer picked "Correct the text". (2026-11-21 fewest-questions triage page (main ca62397de).)
  Maintainer's note on execution: "yes we definitely need to batch these in a much smaller set of plans. please try to
  separate them by area of code so there is a good chance of them running concurrently."
- Completion criteria: Correct the comment, doc or spec prose so it matches the accepted current behavior, as the
  Assessment describes. No behavior change. Remove the feedback file and its index entry.
- Execution: planned in `plans/queue/triage-supervisor.md`.

## linux-docs-capture-follow-ups.md

- Outcome: `fix code`.
- Assessment: None. Three small follow-ups for the screenshot tooling: the README hero wrapper's Linux default (1x), an
  unreferenced Templates-button crop below the publisher's 2,000-byte minimum, and a template-search crop that catches a
  folder warning. Verified: `scripts/docs-shots/launch-templates.py` still has `templates_button` and the search-shot
  framing. The in-flight plan `docs-shots-targeted-runs.md` does not address these. The maintainer already rated them
  low priority.
- Decision: Q12, the maintainer picked "Correct the text". (2026-11-21 fewest-questions triage page (main ca62397de).)
  Maintainer's note on execution: "yes we definitely need to batch these in a much smaller set of plans. please try to
  separate them by area of code so there is a good chance of them running concurrently."
- Completion criteria: Correct the comment, doc or spec prose so it matches the accepted current behavior, as the
  Assessment describes. No behavior change. Remove the feedback file and its index entry.
- Execution: planned in `plans/queue/triage-tooling.md`.

## systemd-isolation-comments-point-to-active-ledger.md

- Outcome: `fix code`.
- Assessment: None. Test comments tell maintainers to look in FLAKES.md for entries that have moved to
  FLAKES_ARCHIVE.md. Verified: The citations are at e2e/harness.rs:1881, e2e/create_idempotency.rs:634 and
  core.rs:27757. The 2026-11-09 entries are only in FLAKES_ARCHIVE.md (4906, 4958). The same drift also exists at
  core.rs:32560 and :37675, which cite 2026-09-30 entries that are now archived.
- Decision: Q12, the maintainer picked "Correct the text". (2026-11-21 fewest-questions triage page (main ca62397de).)
  Maintainer's note on execution: "yes we definitely need to batch these in a much smaller set of plans. please try to
  separate them by area of code so there is a good chance of them running concurrently."
- Completion criteria: Correct the comment, doc or spec prose so it matches the accepted current behavior, as the
  Assessment describes. No behavior change. Remove the feedback file and its index entry.
- Execution: planned in `plans/queue/triage-supervisor.md`.

## retargeted-ssh-alias-can-forget-live-container.md

- Outcome: `fix spec+code`.
- Assessment: A container session's host is reached through an SSH alias. If the user repoints that alias in their SSH
  config to another machine or account, Delete checks the new machine, finds no container, reports success and drops the
  session from Hosts. The real container keeps running on the old machine with nothing in Farhelm managing it. Verified:
  Confirmed on main. `environment/podman.rs` `place()` compares only the registry route (destination string + config
  path). `destroy`/`remove_container` treat Podman's "absent" on that route as proof. No machine or account identity is
  recorded or checked for Podman places. SPEC.md ~322-326 has identity-change detection for ordinary host connections
  ("never silently merges"), and SPEC.md 5285-5296 treats deliberate setup outside Farhelm as the user's intent. Which
  of the two governs here is unsettled.
- Decision: Q4, the maintainer picked "Apply the ordinary identity rules". (2026-11-21 fewest-questions triage page
  (main ca62397de).) Maintainer's note on execution: "yes we definitely need to batch these in a much smaller set of
  plans. please try to separate them by area of code so there is a good chance of them running concurrently."
- Completion criteria: Container hosts reached over SSH follow the ordinary host identity rules: record the machine and
  account identity when a container is bound, refuse to accept "absent" as proof of deletion under a different identity,
  and refuse twin container hosts that reach the same machine and account. Update SPEC accordingly. Both identity items
  may land together. Remove the feedback file and its index entry.
- Execution: planned in `plans/queue/triage-podman.md`.

## podman-place-probe-bypasses-host-identity-refusal.md

- Outcome: `fix spec+code`.
- Assessment: Two SSH host entries that reach the same machine and account (a LAN address and a tailnet address) are
  both offered as usable container hosts, even though Farhelm's ordinary connection refuses the duplicate entry.
  Verified: Confirmed. `podman_probe.rs` caches readiness per host entry and route, with no supervisor identity or
  endpoint comparison. #4083 already fixed the cross-place orphan Delete. What remains is admitting the twin place.
- Decision: Q4, the maintainer picked "Apply the ordinary identity rules". (2026-11-21 fewest-questions triage page
  (main ca62397de).) Maintainer's note on execution: "yes we definitely need to batch these in a much smaller set of
  plans. please try to separate them by area of code so there is a good chance of them running concurrently."
- Completion criteria: Container hosts reached over SSH follow the ordinary host identity rules: record the machine and
  account identity when a container is bound, refuse to accept "absent" as proof of deletion under a different identity,
  and refuse twin container hosts that reach the same machine and account. Update SPEC accordingly. Both identity items
  may land together. Remove the feedback file and its index entry.
- Execution: planned in `plans/queue/triage-podman.md`.

## automatic-update-misses-development-builds.md

- Outcome: `fix code`.
- Assessment: Automatic update overwrites a remote host running a development build if that build speaks an older
  protocol than the helm. It can also overwrite a development build installed while an automatic update was being
  planned. Verified: Confirmed. `automatic.rs` `eligible()` checks for a development build only in the Connected branch;
  the VersionSkew branch skips that check. The unit test docstring limits the exemption to "development build, same
  protocol", which shows the code is deliberate. SPEC.md ~182-184 excludes "hosts running a development build" with no
  exception. The race in the fresh planning probe is confirmed from source and is very narrow.
- Decision: Q6, the maintainer picked "Never touch a development build". (2026-11-21 fewest-questions triage page (main
  ca62397de).) Maintainer's note on execution: "yes we definitely need to batch these in a much smaller set of plans.
  please try to separate them by area of code so there is a good chance of them running concurrently."
- Completion criteria: Automatic update skips every host running a development build, including one that speaks an older
  protocol, as SPEC already says; update the unit test that pins the old behavior. The planning race in the finding is
  not required. Size gate: about 100 lines of diff (code, tests and docs, not counting this ledger, the queue removal or
  a changelog fragment); over that, execution blocks and brings the item back to the maintainer. Remove the feedback
  file and its index entry.
- Execution: planned in `plans/queue/triage-helm.md`.

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

## desktop-smoke-misparses-space-bearing-run-path.md

- Outcome: `other`.
- Assessment: None in the product. The desktop smoke gate falsely fails if a maintainer points `DESKTOP_SMOKE_DIR` at a
  path containing a space. Verified: Accepted from the file: an unbounded `split()` on /proc/net/unix rows, plus
  unquoted redirections at two other spots.
- Decision: Q14, the maintainer picked "Fix only what corrupts evidence or crashes". (2026-11-21 fewest-questions triage
  page (main ca62397de).) Maintainer's note on execution: "yes we definitely need to batch these in a much smaller set
  of plans. please try to separate them by area of code so there is a good chance of them running concurrently."
- Completion criteria: Fix it if the bug crashes a documented command, loses or misattributes test or release evidence,
  turns a real verdict into a harness failure, or lets private data into a fixture; otherwise make no change. Either way
  remove the feedback file and its index entry, and say in the execution record which way it went. Recommendation for
  reference: No. Discard; the scratch-space convention (`/tmp/farhelm-tmp/<uuid>`) never has spaces.
- Execution: planned in `plans/queue/triage-tooling.md`.

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

## mac-fixture-cleanup-stop-budget.md

- Outcome: `other`.
- Assessment: Maintainer tests only. The real-Mac test's cleanup runs the production stop script under a generic 30 s
  limit. Production gives the same script longer, so cleanup could be cut off and leave the fixture behind. Verified:
  Confirmed (`macos_real_tests.rs` ~L313-332 uses `require_shell`, while production uses `STOP_SUPERVISOR_TIMEOUT`). No
  timeout has been observed.
- Decision: Q14, the maintainer picked "Fix only what corrupts evidence or crashes". (2026-11-21 fewest-questions triage
  page (main ca62397de).) Maintainer's note on execution: "yes we definitely need to batch these in a much smaller set
  of plans. please try to separate them by area of code so there is a good chance of them running concurrently."
- Completion criteria: Fix it if the bug crashes a documented command, loses or misattributes test or release evidence,
  turns a real verdict into a harness failure, or lets private data into a fixture; otherwise make no change. Either way
  remove the feedback file and its index entry, and say in the execution record which way it went. Recommendation for
  reference: Fix the code (tiny): use the same timeout.
- Execution: planned in `plans/queue/triage-helm.md`.

## trailing-environment-test-responders-heartbeat-control-decode.md

- Outcome: `other`.
- Assessment: None. A scripted test supervisor panics on the helm's ordinary 30-second heartbeat. That only matters if a
  test keeps it connected longer than 30 seconds. Verified: Partly fixed. `spawn_seal_responder_holding` now skips
  non-Control frames (agent_requests.rs:17753-17761, from 7c83aed7a #3873). `spawn_launch_recording_responder`
  (:15421-15427) still decodes every frame with `expect`. Its one caller does not use a paused clock, so it would need a
  real run longer than 30 seconds.
- Decision: Q14, the maintainer picked "Fix only what corrupts evidence or crashes". (2026-11-21 fewest-questions triage
  page (main ca62397de).) Maintainer's note on execution: "yes we definitely need to batch these in a much smaller set
  of plans. please try to separate them by area of code so there is a good chance of them running concurrently."
- Completion criteria: Fix it if the bug crashes a documented command, loses or misattributes test or release evidence,
  turns a real verdict into a harness failure, or lets private data into a fixture; otherwise make no change. Either way
  remove the feedback file and its index entry, and say in the execution record which way it went. Recommendation for
  reference: Fix the code by copying the 3-line non-Control skip into the remaining responder.
- Execution: planned in `plans/queue/triage-helm.md`.

## trailing-podman-tests-nested-build-module-registration.md

- Outcome: `other`.
- Assessment: None. The claim was that the Podman ownership, cleanup and private-input tests never compile. Verified:
  refuted. `environment/podman/build.rs:9-11` declares `#[cfg(test)] #[path = "tests.rs"] mod tests;`. The parent record
  itself marks it "refuted / excluded".
- Decision: Q14, the maintainer picked "Fix only what corrupts evidence or crashes". (2026-11-21 fewest-questions triage
  page (main ca62397de).) Maintainer's note on execution: "yes we definitely need to batch these in a much smaller set
  of plans. please try to separate them by area of code so there is a good chance of them running concurrently."
- Completion criteria: Fix it if the bug crashes a documented command, loses or misattributes test or release evidence,
  turns a real verdict into a harness failure, or lets private data into a fixture; otherwise make no change. Either way
  remove the feedback file and its index entry, and say in the execution record which way it went. Recommendation for
  reference: discard
- Execution: planned in `plans/queue/triage-podman.md`.

## trailing-podman-acceptance-fallback-misses-inflight-ssh-bootstrap-storage.md

- Outcome: `other`.
- Assessment: None. If a remote Podman create stalls past 120s before libpod registers it, the real SSH test's fallback
  cleanup can report clean while a storage-only builder remains. Verified: `owned_ids`
  (podman_environments.rs:1545-1560) lists only by label, without `--external`. The trigger depends on an unobserved
  filesystem stall.
- Decision: Q14, the maintainer picked "Fix only what corrupts evidence or crashes". (2026-11-21 fewest-questions triage
  page (main ca62397de).) Maintainer's note on execution: "yes we definitely need to batch these in a much smaller set
  of plans. please try to separate them by area of code so there is a good chance of them running concurrently."
- Completion criteria: Fix it if the bug crashes a documented command, loses or misattributes test or release evidence,
  turns a real verdict into a harness failure, or lets private data into a fixture; otherwise make no change. Either way
  remove the feedback file and its index entry, and say in the execution record which way it went. Recommendation for
  reference: discard
- Execution: planned in `plans/queue/triage-podman.md`.

## trailing-deflake-current-driver-downgrades-historical-recorder-required-tmux-policy.md

- Outcome: `other`.
- Assessment: None. A deflake sweep run with `--base` older than the tmux removal no longer builds or requires that
  revision's pinned tmux, so the old recorder only warns. Verified: still present per the record. `deflake/bin/deflake`
  now uses any working tmux (:1149-1154). This only matters for bases from before 2026-11-12.
- Decision: Q14, the maintainer picked "Fix only what corrupts evidence or crashes". (2026-11-21 fewest-questions triage
  page (main ca62397de).) Maintainer's note on execution: "yes we definitely need to batch these in a much smaller set
  of plans. please try to separate them by area of code so there is a good chance of them running concurrently."
- Completion criteria: Fix it if the bug crashes a documented command, loses or misattributes test or release evidence,
  turns a real verdict into a harness failure, or lets private data into a fixture; otherwise make no change. Either way
  remove the feedback file and its index entry, and say in the execution record which way it went. Recommendation for
  reference: discard
- Execution: planned in `plans/queue/triage-tooling.md`.

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

## failed-demo-rollback-can-remove-previous-preview.md

- Outcome: `other`.
- Assessment: None for users. If the demo-video recorder's rollback itself fails, the previous generated demo output can
  be deleted with the temporary directory. Verified: `scripts/readme_video.py:212-266` keeps backups under the
  `TemporaryDirectory` and `restore` can raise inside the `ExitStack`, after which the directory is removed. Needs a
  forward rename failure plus a failed rename-back in the same directory. Output is regenerable.
- Decision: Q14, the maintainer picked "Fix only what corrupts evidence or crashes". (2026-11-21 fewest-questions triage
  page (main ca62397de).) Maintainer's note on execution: "yes we definitely need to batch these in a much smaller set
  of plans. please try to separate them by area of code so there is a good chance of them running concurrently."
- Completion criteria: Fix it if the bug crashes a documented command, loses or misattributes test or release evidence,
  turns a real verdict into a harness failure, or lets private data into a fixture; otherwise make no change. Either way
  remove the feedback file and its index entry, and say in the execution record which way it went. Recommendation for
  reference: discard
- Execution: planned in `plans/queue/triage-tooling.md`.

## docs-shot-cleanup-hides-primary-error.md

- Outcome: `other`.
- Assessment: None for users. When a docs screenshot capture and its cleanup both fail, the report keeps only the
  cleanup error and later shots may run on an unrestored fixture. Verified: `scripts/docs_shots.py:392-418` runs cleanup
  in `finally` and stores `str(error)` of the last exception; `release-notes.py:70-81` re-raises after `remove_group()`.
  Not covered by `plans/queue/docs-shots-targeted-runs.md`.
- Decision: Q14, the maintainer picked "Fix only what corrupts evidence or crashes". (2026-11-21 fewest-questions triage
  page (main ca62397de).) Maintainer's note on execution: "yes we definitely need to batch these in a much smaller set
  of plans. please try to separate them by area of code so there is a good chance of them running concurrently."
- Completion criteria: Fix it if the bug crashes a documented command, loses or misattributes test or release evidence,
  turns a real verdict into a harness failure, or lets private data into a fixture; otherwise make no change. Either way
  remove the feedback file and its index entry, and say in the execution record which way it went. Recommendation for
  reference: discard
- Execution: planned in `plans/queue/triage-tooling.md`.

## docs-shot-guard-ends-before-cleanup.md

- Outcome: `other`.
- Assessment: None for users. The screenshot runner checks staged session IDs before a shot's cleanup runs, not after.
  Verified: `scripts/docs_shots.py:405-411`. The only current cleanup (group delete) changes no session IDs.
- Decision: Q14, the maintainer picked "Fix only what corrupts evidence or crashes". (2026-11-21 fewest-questions triage
  page (main ca62397de).) Maintainer's note on execution: "yes we definitely need to batch these in a much smaller set
  of plans. please try to separate them by area of code so there is a good chance of them running concurrently."
- Completion criteria: Fix it if the bug crashes a documented command, loses or misattributes test or release evidence,
  turns a real verdict into a harness failure, or lets private data into a fixture; otherwise make no change. Either way
  remove the feedback file and its index entry, and say in the execution record which way it went. Recommendation for
  reference: discard
- Execution: planned in `plans/queue/triage-tooling.md`.

## docs-group-shot-lacks-drawn-membership-check.md

- Outcome: `other`.
- Assessment: None for users. The session-group release screenshot waits for the helm to record membership, not for the
  GUI to draw the row under its group. Verified: `scripts/docs-shots/release-notes.py:64-77` polls `/api/sessions`.
  Whether a published image was wrong is unknown. Release shots get human review.
- Decision: Q14, the maintainer picked "Fix only what corrupts evidence or crashes". (2026-11-21 fewest-questions triage
  page (main ca62397de).) Maintainer's note on execution: "yes we definitely need to batch these in a much smaller set
  of plans. please try to separate them by area of code so there is a good chance of them running concurrently."
- Completion criteria: Fix it if the bug crashes a documented command, loses or misattributes test or release evidence,
  turns a real verdict into a harness failure, or lets private data into a fixture; otherwise make no change. Either way
  remove the feedback file and its index entry, and say in the execution record which way it went. Recommendation for
  reference: discard
- Execution: planned in `plans/queue/triage-tooling.md`.

## docs-group-shot-wait-exceeds-budget.md

- Outcome: `other`.
- Assessment: None for users. A stalled staged helm can make the group shot's 10-second wait last about a minute.
  Verified: the probe uses a request helper with its own timeout (`release-notes.py:64-77`); the poll checks elapsed
  time only between probes. Helper timeout value not re-read.
- Decision: Q14, the maintainer picked "Fix only what corrupts evidence or crashes". (2026-11-21 fewest-questions triage
  page (main ca62397de).) Maintainer's note on execution: "yes we definitely need to batch these in a much smaller set
  of plans. please try to separate them by area of code so there is a good chance of them running concurrently."
- Completion criteria: Fix it if the bug crashes a documented command, loses or misattributes test or release evidence,
  turns a real verdict into a harness failure, or lets private data into a fixture; otherwise make no change. Either way
  remove the feedback file and its index entry, and say in the execution record which way it went. Recommendation for
  reference: discard
- Execution: planned in `plans/queue/triage-tooling.md`.

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

## child-hangup-masks-missing-stop-tree-sweep.md

- Outcome: `other`.
- Assessment: None in the product. The Stop test's background child would also die from the kernel's hangup when its
  parent shell is killed, so the test cannot prove Stop swept the whole tree. Verified: Accepted from the file. A
  hangup-immune fixture exists, and the Restart and Delete tests already use it (e2e host_terminals.rs:388-389).
- Decision: Q11, the maintainer picked "Fix only the cheap ones that guard product behavior". (2026-11-21
  fewest-questions triage page (main ca62397de).) Maintainer's note on execution: "yes we definitely need to batch these
  in a much smaller set of plans. please try to separate them by area of code so there is a good chance of them running
  concurrently."
- Completion criteria: Fix the test if the fix is a few lines (about 20 at most) and the test guards a user-facing
  behavior, including a retired test whose behavior still applies; otherwise make no change. Either way remove the
  feedback file and its index entry, and say in the execution record which way it went. Recommendation for reference:
  Fix in a batch by switching Stop's test to the hangup-immune tree.
- Execution: planned in `plans/queue/triage-supervisor.md`.

## restart-test-counts-zombie-as-running-agent.md

- Outcome: `other`.
- Assessment: None in the product. The Restart ordering test can falsely fail if the old grandchild is an exited but not
  yet reaped process when the new host starts, because `kill -0` counts zombies as alive. Verified: e2e
  host_terminals.rs:359-364 uses `kill -0` and expects " gone gone" (confirmed). The harness and production both treat
  zombies as gone. No flake is recorded. The window depends on the orphan reaper and is likely short.
- Decision: Q11, the maintainer picked "Fix only the cheap ones that guard product behavior". (2026-11-21
  fewest-questions triage page (main ca62397de).) Maintainer's note on execution: "yes we definitely need to batch these
  in a much smaller set of plans. please try to separate them by area of code so there is a good chance of them running
  concurrently."
- Completion criteria: Fix the test if the fix is a few lines (about 20 at most) and the test guards a user-facing
  behavior, including a retired test whose behavior still applies; otherwise make no change. Either way remove the
  feedback file and its index entry, and say in the execution record which way it went. Recommendation for reference:
  Fix code: check process state (Z counts as gone) in the wrapper.
- Execution: planned in `plans/queue/triage-supervisor.md`.

## restart-screen-captured-before-prompt-settles.md

- Outcome: `other`.
- Assessment: None in the product. The orderly-restart test may capture its expected screen before the fake agent prints
  its prompt, and so fail falsely. Verified: host_restarts.rs:316-322 waits only for "echo:before-restart" and then
  captures `screen_tail` (confirmed). The fake agent writing the prompt separately is taken from the file. No flake is
  recorded.
- Decision: Q11, the maintainer picked "Fix only the cheap ones that guard product behavior". (2026-11-21
  fewest-questions triage page (main ca62397de).) Maintainer's note on execution: "yes we definitely need to batch these
  in a much smaller set of plans. please try to separate them by area of code so there is a good chance of them running
  concurrently."
- Completion criteria: Fix the test if the fix is a few lines (about 20 at most) and the test guards a user-facing
  behavior, including a retired test whose behavior still applies; otherwise make no change. Either way remove the
  feedback file and its index entry, and say in the execution record which way it went. Recommendation for reference:
  Fix code: wait for the prompt after the echo before capturing.
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

## last-output-fixture-readiness-precedes-trap.md

- Outcome: `other`.
- Assessment: None. The test that a restart keeps the old program's last output can fail spuriously, because the fake
  agent prints its ready marker before it installs the TERM trap that prints those last words. Verified: Confirmed on
  main (`e2e/host_relaunches.rs` `history_agent_with`: `OLD_SCREEN_MARK` is printed before `trap ... TERM HUP`). The
  test restarts immediately after seeing the marker. Not in FLAKES.md, and no observed flake.
- Decision: Q11, the maintainer picked "Fix only the cheap ones that guard product behavior". (2026-11-21
  fewest-questions triage page (main ca62397de).) Maintainer's note on execution: "yes we definitely need to batch these
  in a much smaller set of plans. please try to separate them by area of code so there is a good chance of them running
  concurrently."
- Completion criteria: Fix the test if the fix is a few lines (about 20 at most) and the test guards a user-facing
  behavior, including a retired test whose behavior still applies; otherwise make no change. Either way remove the
  feedback file and its index entry, and say in the execution record which way it went. Recommendation for reference:
  fix code. Move the trap ahead of the marker in the fixture script (a one-line reorder).
- Execution: planned in `plans/queue/triage-supervisor.md`.

## host-preservation-test-can-select-starter-pid.md

- Outcome: `other`.
- Assessment: None. The test that startup reclaim leaves a reserved session's terminal host running can record the
  short-lived first-stage launcher's PID and later fail when that launcher exits normally. Verified: Still present:
  `host_process` (`e2e/host_terminals.rs`) returns the first `/proc` entry whose argv has `--socket <path>`, which also
  matches the first stage. `/proc` lists PIDs in ascending order, so the lower-numbered first stage wins while both
  processes exist. In practice, creating the second session first makes the overlap unlikely. Not in FLAKES.md.
- Decision: Q11, the maintainer picked "Fix only the cheap ones that guard product behavior". (2026-11-21
  fewest-questions triage page (main ca62397de).) Maintainer's note on execution: "yes we definitely need to batch these
  in a much smaller set of plans. please try to separate them by area of code so there is a good chance of them running
  concurrently."
- Completion criteria: Fix the test if the fix is a few lines (about 20 at most) and the test guards a user-facing
  behavior, including a retired test whose behavior still applies; otherwise make no change. Either way remove the
  feedback file and its index entry, and say in the execution record which way it went. Recommendation for reference:
  fix code, small (wait until exactly one matching process remains, or match the daemon specifically).
- Execution: planned in `plans/queue/triage-supervisor.md`.

## restart-test-types-before-safe-prompt-barrier.md

- Outcome: `other`.
- Assessment: None. The supervisor-restart survival tests type input after `FAKE-AGENT READY` rather than after the `> `
  prompt. The fixture's own documentation says to wait for the prompt, because input echoed between CR and LF can
  overwrite the marker row. Verified: Confirmed (`e2e/host_processes.rs` `a_host_agent_survives` waits for
  `FAKE-AGENT READY` and then calls `type_line`. `fake_agent.rs` documents `> ` as the SAFE-TO-TYPE barrier). The rule
  lives in fixture docs, not in a spec. No observed flake.
- Decision: Q11, the maintainer picked "Fix only the cheap ones that guard product behavior". (2026-11-21
  fewest-questions triage page (main ca62397de).) Maintainer's note on execution: "yes we definitely need to batch these
  in a much smaller set of plans. please try to separate them by area of code so there is a good chance of them running
  concurrently."
- Completion criteria: Fix the test if the fix is a few lines (about 20 at most) and the test guards a user-facing
  behavior, including a retired test whose behavior still applies; otherwise make no change. Either way remove the
  feedback file and its index entry, and say in the execution record which way it went. Recommendation for reference:
  fix code. Wait for the documented `> ` barrier before typing.
- Execution: planned in `plans/queue/triage-supervisor.md`.

## upload-failure-test-lacks-pipe-holder-witness.md

- Outcome: `other`.
- Assessment: None. The test that a failed host upload does not hang can pass without the pipe-holding helper ever
  starting, so it may not test the original hang. Verified: Confirmed (`provisioning/backend.rs`
  `a_failed_upload_does_not_wait_on_a_blocked_feeder`). The holder is backgrounded and the shell exits 7 at once, with
  no readiness signal and no check that the feeder actually blocked. The test's own NOTE says the shape is defensive and
  not measured from OpenSSH.
- Decision: Q11, the maintainer picked "Fix only the cheap ones that guard product behavior". (2026-11-21
  fewest-questions triage page (main ca62397de).) Maintainer's note on execution: "yes we definitely need to batch these
  in a much smaller set of plans. please try to separate them by area of code so there is a good chance of them running
  concurrently."
- Completion criteria: Fix the test if the fix is a few lines (about 20 at most) and the test guards a user-facing
  behavior, including a retired test whose behavior still applies; otherwise make no change. Either way remove the
  feedback file and its index entry, and say in the execution record which way it went. Recommendation for reference:
  discard. The test covers a defensive shape, and making it a strong oracle takes nontrivial fixture work.
- Execution: planned in `plans/queue/triage-helm.md`.

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

## stop-annotation-exit-witness.md

- Outcome: `other`.
- Assessment: Maintainer tests only. A test lists the session immediately after Stop returns and expects "stopped by
  user". The host's exit record can arrive after Stop returns, so the listing may still show the session live. Verified:
  Confirmed. `restart_under_concurrency.rs` L223-225 lists immediately. In `sweep.rs` `stop_live_agent`, a comment
  explicitly allows the exit record to lag. `status.rs` hides the annotation while the host still reads live. No flake
  has been logged.
- Decision: Q11, the maintainer picked "Fix only the cheap ones that guard product behavior". (2026-11-21
  fewest-questions triage page (main ca62397de).) Maintainer's note on execution: "yes we definitely need to batch these
  in a much smaller set of plans. please try to separate them by area of code so there is a good chance of them running
  concurrently."
- Completion criteria: Fix the test if the fix is a few lines (about 20 at most) and the test guards a user-facing
  behavior, including a retired test whose behavior still applies; otherwise make no change. Either way remove the
  feedback file and its index entry, and say in the execution record which way it went. Recommendation for reference:
  Fix the code: one line, `wait_for_dead_agent` before the assertion.
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

## mac-uninstall-test-lacks-live-host-premise.md

- Outcome: `other`.
- Assessment: Maintainer tests only (the real-Mac test). The uninstall check would still pass if a regression ended the
  retained agent terminal host at Stop, because nothing confirms that host is alive before uninstall. Verified:
  Confirmed. `macos_real_tests.rs` ~L736-762 checks only that processes are gone after uninstall, against identities
  captured before Stop.
- Decision: Q11, the maintainer picked "Fix only the cheap ones that guard product behavior". (2026-11-21
  fewest-questions triage page (main ca62397de).) Maintainer's note on execution: "yes we definitely need to batch these
  in a much smaller set of plans. please try to separate them by area of code so there is a good chance of them running
  concurrently."
- Completion criteria: Fix the test if the fix is a few lines (about 20 at most) and the test guards a user-facing
  behavior, including a retired test whose behavior still applies; otherwise make no change. Either way remove the
  feedback file and its index entry, and say in the execution record which way it went. Recommendation for reference:
  Fix the code (small): assert that host is alive after Stop.
- Execution: planned in `plans/queue/triage-helm.md`.

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

## trailing-nonfinal-environment-delete-cache-survivor-oracle.md

- Outcome: `other`.
- Assessment: None. A test for deleting one session from an environment would still pass if the helm forgot every
  session on that host. Verified: agent_requests.rs:17990-18001 asserts only `!cached.contains("helper")`, never that
  "starter" remains.
- Decision: Q11, the maintainer picked "Fix only the cheap ones that guard product behavior". (2026-11-21
  fewest-questions triage page (main ca62397de).) Maintainer's note on execution: "yes we definitely need to batch these
  in a much smaller set of plans. please try to separate them by area of code so there is a good chance of them running
  concurrently."
- Completion criteria: Fix the test if the fix is a few lines (about 20 at most) and the test guards a user-facing
  behavior, including a retired test whose behavior still applies; otherwise make no change. Either way remove the
  feedback file and its index entry, and say in the execution record which way it went. Recommendation for reference:
  Fix the code: one assertion that the cache equals ["starter"].
- Execution: planned in `plans/queue/triage-helm.md`.

## trailing-stopped-episode-test-does-not-witness-settled-unavailable-cycle.md

- Outcome: `other`.
- Assessment: None. The test that a stopped container gets one automatic start and keeps its failure across unanswered
  status reads can pass without ever completing a failed connection cycle. Verified: manager.rs:14533-14536 waits for
  `checked + 2` provider calls and then calls `retry_now`. It never waits for a settled Unreachable.
- Decision: Q11, the maintainer picked "Fix only the cheap ones that guard product behavior". (2026-11-21
  fewest-questions triage page (main ca62397de).) Maintainer's note on execution: "yes we definitely need to batch these
  in a much smaller set of plans. please try to separate them by area of code so there is a good chance of them running
  concurrently."
- Completion criteria: Fix the test if the fix is a few lines (about 20 at most) and the test guards a user-facing
  behavior, including a retired test whose behavior still applies; otherwise make no change. Either way remove the
  feedback file and its index entry, and say in the execution record which way it went. Recommendation for reference:
  Fix the code (wait for a settled failure, or use a one-attempt cadence). Discarding is also reasonable.
- Execution: planned in `plans/queue/triage-podman.md`.

## trailing-host-phase-vocabulary-test-omits-stopped.md

- Outcome: `other`.
- Assessment: None. The test that keeps the agent-facing phase word equal to the wire tag claims to cover "every
  variant", but it leaves out Stopped and Deleting. Verified: The hosts.rs:1619-1691 matrix has no Stopped or Deleting
  entry.
- Decision: Q11, the maintainer picked "Fix only the cheap ones that guard product behavior". (2026-11-21
  fewest-questions triage page (main ca62397de).) Maintainer's note on execution: "yes we definitely need to batch these
  in a much smaller set of plans. please try to separate them by area of code so there is a good chance of them running
  concurrently."
- Completion criteria: Fix the test if the fix is a few lines (about 20 at most) and the test guards a user-facing
  behavior, including a retired test whose behavior still applies; otherwise make no change. Either way remove the
  feedback file and its index entry, and say in the execution record which way it went. Recommendation for reference:
  Fix the code: add the two fixtures.
- Execution: planned in `plans/queue/triage-helm.md`.

## trailing-cancelled-trash-observation-test-misses-fleet-publication.md

- Outcome: `other`.
- Assessment: None. The cancellation test for Trash reads checks the per-host counter, but not the fleet event that
  tells other windows to refresh. Verified: notices.rs:1179-1221 asserts the epoch only, with no fleet revision.
- Decision: Q11, the maintainer picked "Fix only the cheap ones that guard product behavior". (2026-11-21
  fewest-questions triage page (main ca62397de).) Maintainer's note on execution: "yes we definitely need to batch these
  in a much smaller set of plans. please try to separate them by area of code so there is a good chance of them running
  concurrently."
- Completion criteria: Fix the test if the fix is a few lines (about 20 at most) and the test guards a user-facing
  behavior, including a retired test whose behavior still applies; otherwise make no change. Either way remove the
  feedback file and its index entry, and say in the execution record which way it went. Recommendation for reference:
  Fix the code (capture the revision before and assert after). Discarding is also reasonable.
- Execution: planned in `plans/queue/triage-helm.md`.

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

## trailing-podman-probe-dedup-test-asserts-before-spawned-task.md

- Outcome: `other`.
- Assessment: None. The container-host probe deduplication test asserts its call count before a duplicate spawned task
  could run, so it would pass a duplicating regression. Verified: still at `environment/podman_probe.rs:992-993`: the
  second refresh is followed immediately by a calls==1 assert, with no join of later tasks.
- Decision: Q11, the maintainer picked "Fix only the cheap ones that guard product behavior". (2026-11-21
  fewest-questions triage page (main ca62397de).) Maintainer's note on execution: "yes we definitely need to batch these
  in a much smaller set of plans. please try to separate them by area of code so there is a good chance of them running
  concurrently."
- Completion criteria: Fix the test if the fix is a few lines (about 20 at most) and the test guards a user-facing
  behavior, including a retired test whose behavior still applies; otherwise make no change. Either way remove the
  feedback file and its index entry, and say in the execution record which way it went. Recommendation for reference:
  discard, or batch into one test-hardening PR if the maintainer values it
- Execution: planned in `plans/queue/triage-podman.md`.

## trailing-podman-proxy-exact-inheritance-test-shared-serializer-oracle.md

- Outcome: `other`.
- Assessment: None. The proxy privacy test computes its expected bytes with the production serializer, so a lossy
  serializer change would pass. Verified: still at `environment/podman/failure_tests.rs:1033`
  (`build::proxy_exports(&values)` as the oracle).
- Decision: Q11, the maintainer picked "Fix only the cheap ones that guard product behavior". (2026-11-21
  fewest-questions triage page (main ca62397de).) Maintainer's note on execution: "yes we definitely need to batch these
  in a much smaller set of plans. please try to separate them by area of code so there is a good chance of them running
  concurrently."
- Completion criteria: Fix the test if the fix is a few lines (about 20 at most) and the test guards a user-facing
  behavior, including a retired test whose behavior still applies; otherwise make no change. Either way remove the
  feedback file and its index entry, and say in the execution record which way it went. Recommendation for reference:
  discard or batch (or narrow the comment)
- Execution: planned in `plans/queue/triage-podman.md`.

## trailing-podman-delete-lifecycle-test-equal-membership-oracle.md

- Outcome: `other`.
- Assessment: None. The real-container Delete test claims a helm that counted its own list would fail it, but its helm
  and container hold the same membership. Verified: comment still at
  `crates/farhelm/tests/e2e/podman_environments.rs:1469-1471`. Production uses the supervisor's answer.
- Decision: Q11, the maintainer picked "Fix only the cheap ones that guard product behavior". (2026-11-21
  fewest-questions triage page (main ca62397de).) Maintainer's note on execution: "yes we definitely need to batch these
  in a much smaller set of plans. please try to separate them by area of code so there is a good chance of them running
  concurrently."
- Completion criteria: Fix the test if the fix is a few lines (about 20 at most) and the test guards a user-facing
  behavior, including a retired test whose behavior still applies; otherwise make no change. Either way remove the
  feedback file and its index entry, and say in the execution record which way it went. Recommendation for reference:
  discard, or narrow the comment in the doc batch
- Execution: planned in `plans/queue/triage-podman.md`.

## trailing-podman-lost-session-create-test-misses-presend-store-witness.md

- Outcome: `other`.
- Assessment: None. The lost-create test says login and install facts become durable before run, but it reads them only
  after create returns. Verified: the comment is still at `environment/podman/tests.rs:273`, and the fake send has no
  session-store oracle. Production ordering is correct.
- Decision: Q11, the maintainer picked "Fix only the cheap ones that guard product behavior". (2026-11-21
  fewest-questions triage page (main ca62397de).) Maintainer's note on execution: "yes we definitely need to batch these
  in a much smaller set of plans. please try to separate them by area of code so there is a good chance of them running
  concurrently."
- Completion criteria: Fix the test if the fix is a few lines (about 20 at most) and the test guards a user-facing
  behavior, including a retired test whose behavior still applies; otherwise make no change. Either way remove the
  feedback file and its index entry, and say in the execution record which way it went. Recommendation for reference:
  discard or batch (or narrow the comment)
- Execution: planned in `plans/queue/triage-podman.md`.

## trailing-real-container-agent-host-list-oracle-omits-extra-ssh-hosts.md

- Outcome: `other`.
- Assessment: None. The real-container test says the hosts listing shows only the container, but it only checks
  substring inclusion and exclusion. Verified: still at `podman_environments.rs:700` (claim) and :816-818
  (`hosts.contains(own_name)`).
- Decision: Q11, the maintainer picked "Fix only the cheap ones that guard product behavior". (2026-11-21
  fewest-questions triage page (main ca62397de).) Maintainer's note on execution: "yes we definitely need to batch these
  in a much smaller set of plans. please try to separate them by area of code so there is a good chance of them running
  concurrently."
- Completion criteria: Fix the test if the fix is a few lines (about 20 at most) and the test guards a user-facing
  behavior, including a retired test whose behavior still applies; otherwise make no change. Either way remove the
  feedback file and its index entry, and say in the execution record which way it went. Recommendation for reference:
  discard or batch (parse the rows, or narrow the comment)
- Execution: planned in `plans/queue/triage-podman.md`.

## trailing-missing-release-entry-test-count-does-not-prove-exact-variant-inventory.md

- Outcome: `other`.
- Assessment: None. A fixture premise check says the variant "leaves out X and nothing else" but checks only absence
  plus count. Verified: still at `provisioning/release_payloads.rs:2528-2529`.
- Decision: Q11, the maintainer picked "Fix only the cheap ones that guard product behavior". (2026-11-21
  fewest-questions triage page (main ca62397de).) Maintainer's note on execution: "yes we definitely need to batch these
  in a much smaller set of plans. please try to separate them by area of code so there is a good chance of them running
  concurrently."
- Completion criteria: Fix the test if the fix is a few lines (about 20 at most) and the test guards a user-facing
  behavior, including a retired test whose behavior still applies; otherwise make no change. Either way remove the
  feedback file and its index entry, and say in the execution record which way it went. Recommendation for reference:
  discard or batch
- Execution: planned in `plans/queue/triage-helm.md`.

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

## trailing-failed-restart-loses-the-live-tab-preservation-regression-oracle.md

- Outcome: `other`.
- Assessment: A Restart that fails must leave the session's open shell tabs and their work running. The test proving
  that went away with tmux. No test now opens a tab, makes a Restart fail, and checks the tab's shell survived.
  Verified: crates/farhelm/tests/e2e/host_relaunches.rs on main: the only tabs are opened in the successful-Restart test
  (:287) and the stop-during-restart helper (:375). The refused/unusable replacement tests (:595, :697) open no tab.
  Production behavior was not alleged wrong. SPEC.md:2042 says "Restart ... leave[s] tabs running".
- Decision: Q11, the maintainer picked "Fix only the cheap ones that guard product behavior". (2026-11-21
  fewest-questions triage page (main ca62397de).) Maintainer's note on execution: "yes we definitely need to batch these
  in a much smaller set of plans. please try to separate them by area of code so there is a good chance of them running
  concurrently."
- Completion criteria: Fix the test if the fix is a few lines (about 20 at most) and the test guards a user-facing
  behavior, including a retired test whose behavior still applies; otherwise make no change. Either way remove the
  feedback file and its index entry, and say in the execution record which way it went. Recommendation for reference:
  fix code (add a tab to an existing failed-replacement fixture and check its shell pid/identity survives)
- Execution: planned in `plans/queue/triage-supervisor.md`.

## trailing-deleted-ambiguous-create-test-loses-published-handle-and-pending-intent-oracles.md

- Outcome: `other`.
- Assessment: When a create fails and Farhelm can't tell whether the agent started, the session must stay listed and
  reachable for Stop/Delete, and a retry with the same key must not be treated as finished. The host version of the test
  only checks that a database row survives. Verified: host_creates.rs
  `an_unusable_host_that_cannot_be_ended_keeps_the_launching_record` (~:1169) still calls an unkeyed `create_session`
  and never lists the session or checks the reservation. Production is said to be correct (not re-verified in depth).
- Decision: Q11, the maintainer picked "Fix only the cheap ones that guard product behavior". (2026-11-21
  fewest-questions triage page (main ca62397de).) Maintainer's note on execution: "yes we definitely need to batch these
  in a much smaller set of plans. please try to separate them by area of code so there is a good chance of them running
  concurrently."
- Completion criteria: Fix the test if the fix is a few lines (about 20 at most) and the test guards a user-facing
  behavior, including a retired test whose behavior still applies; otherwise make no change. Either way remove the
  feedback file and its index entry, and say in the execution record which way it went. Recommendation for reference:
  fix code (add a key, assert listed and reservation still pending)
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

## trailing-removed-completed-keyed-create-reboot-replay-oracle.md

- Outcome: `other`.
- Assessment: None directly. Retrying a keyed create after a host reboot must return the original session, not start
  another agent. The only test of that went away with the terminal-kind field removal. Verified: create_idempotency.rs
  on main has only the never-launched reboot case (:839) and same-boot restart. No completed-create reboot retry.
  SPEC.md keeps retry identity for the child's lifetime.
- Decision: Q11, the maintainer picked "Fix only the cheap ones that guard product behavior". (2026-11-21
  fewest-questions triage page (main ca62397de).) Maintainer's note on execution: "yes we definitely need to batch these
  in a much smaller set of plans. please try to separate them by area of code so there is a good chance of them running
  concurrently."
- Completion criteria: Fix the test if the fix is a few lines (about 20 at most) and the test guards a user-facing
  behavior, including a retired test whose behavior still applies; otherwise make no change. Either way remove the
  feedback file and its index entry, and say in the execution record which way it went. Recommendation for reference:
  fix code (restore the deleted sequence minus the kind fields)
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

## connection-refusal-tests-release-port-too-early.md

- Outcome: `other`.
- Assessment: None. Two tests bind port 0, drop the listener, then connect expecting a refusal; another process could
  grab the port in between. Verified: `farhelm-desktop/src/updater/transport_tests.rs:170-183` still `drop(listener)`
  before connecting; transfers test similar. Consequence of the race is a spurious test failure, not a false pass. Not
  covered by the AGENTS.md fixed-port rule (the port is kernel-chosen).
- Decision: Q11, the maintainer picked "Fix only the cheap ones that guard product behavior". (2026-11-21
  fewest-questions triage page (main ca62397de).) Maintainer's note on execution: "yes we definitely need to batch these
  in a much smaller set of plans. please try to separate them by area of code so there is a good chance of them running
  concurrently."
- Completion criteria: Fix the test if the fix is a few lines (about 20 at most) and the test guards a user-facing
  behavior, including a retired test whose behavior still applies; otherwise make no change. Either way remove the
  feedback file and its index entry, and say in the execution record which way it went. Recommendation for reference:
  discard (or fix: keep a bound non-listening socket, a few lines each)
- Execution: planned in `plans/queue/triage-helm.md`.

## devbuild-startup-test-misses-first-scan.md

- Outcome: `other`.
- Assessment: None directly. The devbuild startup test checks the stored update choice only after the helm exits, so it
  would not catch the first automatic scan running before the seed. Verified: test at
  `farhelm-helm/src/lib.rs:1897-1953`; production seeds before serving (1405-1421), as the finding concedes.
- Decision: Q11, the maintainer picked "Fix only the cheap ones that guard product behavior". (2026-11-21
  fewest-questions triage page (main ca62397de).) Maintainer's note on execution: "yes we definitely need to batch these
  in a much smaller set of plans. please try to separate them by area of code so there is a good chance of them running
  concurrently."
- Completion criteria: Fix the test if the fix is a few lines (about 20 at most) and the test guards a user-facing
  behavior, including a retired test whose behavior still applies; otherwise make no change. Either way remove the
  feedback file and its index entry, and say in the execution record which way it went. Recommendation for reference:
  discard
- Execution: planned in `plans/queue/triage-helm.md`.

## automatic-update-wake-test-accepts-host-refresh.md

- Outcome: `other`.
- Assessment: None directly. The test that clearing the opt-out wakes host auto-updates could be satisfied by a late
  first host refresh instead. Verified: `farhelm-helm/src/provisioning.rs:1980-2053` waits for Connected and one
  completed scan, not for the initial collections read. Alternate wake producer not exercised.
- Decision: Q11, the maintainer picked "Fix only the cheap ones that guard product behavior". (2026-11-21
  fewest-questions triage page (main ca62397de).) Maintainer's note on execution: "yes we definitely need to batch these
  in a much smaller set of plans. please try to separate them by area of code so there is a good chance of them running
  concurrently."
- Completion criteria: Fix the test if the fix is a few lines (about 20 at most) and the test guards a user-facing
  behavior, including a retired test whose behavior still applies; otherwise make no change. Either way remove the
  feedback file and its index entry, and say in the execution record which way it went. Recommendation for reference:
  discard
- Execution: planned in `plans/queue/triage-helm.md`.

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

## desktop-path-notice-first-read-untested.md

- Outcome: `other`.
- Assessment: None. No test checks the whole path from the desktop's failed PATH lookup to the first client seeing the
  notice before the helm serves. Verified: I accepted the file's citations and did not re-trace them. SPEC_impl.md ~3925
  requires the handoff before serving. Covering it would need a new desktop or embedded-helm test across components.
- Decision: Q11, the maintainer picked "Fix only the cheap ones that guard product behavior". (2026-11-21
  fewest-questions triage page (main ca62397de).) Maintainer's note on execution: "yes we definitely need to batch these
  in a much smaller set of plans. please try to separate them by area of code so there is a good chance of them running
  concurrently."
- Completion criteria: Fix the test if the fix is a few lines (about 20 at most) and the test guards a user-facing
  behavior, including a retired test whose behavior still applies; otherwise make no change. Either way remove the
  feedback file and its index entry, and say in the execution record which way it went. Recommendation for reference:
  discard (no defect, and the test would cost more than a tiny assertion)
- Execution: planned in `plans/queue/triage-helm.md`.

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

## trailing-podman-pending-commit-tag-no-dispatch-crash-never-settles.md

- Outcome: `fix code`.
- Assessment: If the helm stops at the wrong moment during an image capture (or the capture's outcome is otherwise never
  confirmed), the recipe can never be deleted and its container host cannot be removed or retargeted; nothing in the UI
  clears it. Verified: build.rs `retire_images_unlocked` (~758): an absent pending tag fails with "an unanswered image
  capture has no confirmed outcome; its ownership tag was retained", with no settle path. This is a helm crash, not the
  supervisor crash the 2026-10-10 acceptance names, and the result is lasting.
- Decision: returned from Q8 as not straightforward; on the follow-up page the maintainer picked "Offer an explicit
  "delete anyway"" (2026-11-21 fewest-questions triage, main ca62397de). Maintainer's note on execution: "yes we
  definitely need to batch these in a much smaller set of plans. please try to separate them by area of code so there is
  a good chance of them running concurrently."
- Completion criteria: When a recipe's image capture outcome is unknown and the tagged image is absent, recipe Delete
  asks for an explicit confirmation and then settles the pending marker, so the recipe can be deleted and its container
  host removed or retargeted. Medium; the maintainer accepted the new confirmation path. Remove the feedback file and
  its index entry.
- Execution: planned in `plans/queue/triage-podman.md`.

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

## late-delete-loses-checkout-archive-notice.md

- Outcome: `fix code`.
- Assessment: If a session Delete takes longer than 20 s, Farhelm later removes the session but never says that its
  checkout was left unarchived, or where. Verified: the helm bounds the Delete route at `HOST_REPLY_BOUND` = 20 s
  (`farhelm-helm/src/sessions.rs:3767, 4029-4038`), while Delete's own budgets run to 60+60+15 s, so a slow Delete is
  ordinary rather than rare. The GUI publishes the notice only from a `Done` reply (`session_list/delete.rs:378-386`).
  No late delivery exists. SPEC.md ~1986-1988: "That outcome must never be silent: Delete completes with a visible
  notice ... Wherever Delete's result is shown, the notice is shown with it."
- Decision: returned from Q8 as not straightforward; on the follow-up page the maintainer picked "Deliver the late
  notice" (2026-11-21 fewest-questions triage, main ca62397de). Maintainer's note on execution: "yes we definitely need
  to batch these in a much smaller set of plans. please try to separate them by area of code so there is a good chance
  of them running concurrently."
- Completion criteria: When a session Delete finishes after its helm reply timed out, publish its checkout-archive
  notice (unarchived, and where) through the notifications feed, so SPEC's "must never be silent" holds. Medium;
  accepted. Remove the feedback file and its index entry.
- Execution: planned in `plans/queue/triage-helm.md`.

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

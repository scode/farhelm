# Template approval cards and summaries misstate unsupported stored permissions for non-shell agents

Reviewed commit: 193043abdff70fe3c7b27d081620d64d0c8c5302

## TLDR

When an agent asks to create or edit a launch template with an approval mode that the template's chosen agent does not
support (for example Custom on Pi, OpenCode, or Goose, Approve/Chat on Claude or Codex, or Guarded/Approve on Cursor),
the approval card and template list summary still show that agent's default approval mode instead of the mode the
template would actually store.

## Details

**other.** Source inspection. Main reference `crates/gui/farhelm-gui/src/templates/summary.rs:187`.

Commit 193043abdff70fe3c7b27d081620d64d0c8c5302 fixed crates/gui/farhelm-gui/src/templates/summary.rs:182-186 so
permission_label() displays the actual stored permission for shell harnesses (Bash/Zsh) instead of returning "None
(shell)". However, for every non-shell harness, crates/gui/farhelm-gui/src/templates/summary.rs:187-193 still calls
harness.request_permission(permission). In crates/farhelm-proto/src/launch.rs:367-375,
LaunchHarness::request_permission() replaces any explicit Some(value) where !self.offers_permission(value) with
self.omitted_permission(). Because agent template writes (crates/farhelm-helm/src/agent_requests.rs:3140-3266) validate
only check_template_shape() and never offers_permission(), an agent can propose (or an existing template can store) an
incompatible explicit permission for a non-shell harness, and crates/gui/farhelm-gui/src/approvals/model.rs:801-808
(TemplateWrite, TemplateWriteWithShortcuts, TemplateDelete) and templates/summary.rs:92-97 render it through
permission_label(). Concrete mismatches against crates/farhelm-proto/src/launch.rs:327-340 and 508-543: (1) Pi/OpenCode
with Custom/Guarded/Approve/SmartApprove/Chat displays "YOLO"; (2) Goose with Custom/Guarded displays "YOLO"; (3)
Claude/Codex/Muse/Grok/Antigravity with Approve/SmartApprove/Chat (and Omp with SmartApprove/Chat) displays the
harness's guarded mode name; (4) Cursor with Guarded/Approve/SmartApprove/Chat displays "Agent's own setting". This
violates SPEC.md ("Agent-spawned sessions": "template cards keep their stored permission labels and YOLO assertions").
Note: the Templates editor (templates/editor/rows.rs:365,539) already appends "<word> (not offered)" and keeps
CustomFlags visible for non-shell harnesses; only summary::permission_label() misstates them. Fix direction: in
permission_label(), use harness.omitted_permission() only when permission is None (an explicit reset); when permission
is Some(LaunchPermission::Guarded) and harness.offers_permission(LaunchPermission::Guarded) use
guarded_mode_name(harness), and for all other Some(mode) return permission_display(Some(mode)) directly (which handles
shells and non-shells uniformly).

Reviewer metadata (g05-triage-gui-templates-hosts/l2): suggested bucket: other (could NOT be highest on shipped
harnesses: every non-shell harness offers LaunchPermission::Yolo, every non-shell harness that does not offer Custom
falls back to YOLO rather than a safer label, and approvals/model.rs:812 unconditionally displays permission_flags
whenever present); confidence: confirmed; possible cover: none.

Review evidence is retained under run `34570dac-b635-4682-a942-3a3fc1728e12` as `g05-triage-gui-templates-hosts/l2`.

Additional source record at `1b3fe49af86979aebc2719965682a17f8c6366ac`,
`crates/gui/farhelm-gui/src/templates/summary.rs:192` (g20_yolo_unset_gui_launcher/l4):

Follow-up observation at `1b3fe49af86979aebc2719965682a17f8c6366ac` after removing `Agent's own setting`
(`#4673`–`#4676`): Cursor's `omitted_permission()` is now `Some(LaunchPermission::Yolo)`, so unsupported modes on Cursor
now misdisplay as `YOLO` instead of `Agent's own setting`.

In crates/gui/farhelm-gui/src/templates/summary.rs:177-199, `permission_label(agent, permission)` handles shells
verbatim (lines 187-191) so an incompatible stored permission like `Custom` on `Bash` still reads `"Custom"`, but for
every non-shell harness line 192 calls `harness.request_permission(permission)`. In
crates/farhelm-proto/src/launch.rs:421-429, `request_permission(Some(value))` checks `if self.offers_permission(value)`
and otherwise falls back to `self.omitted_permission()`. Because `check_template_shape`
(crates/farhelm-proto/src/launcher.rs:546-643) does not reject incompatible `(agent, permissions)` pairs, an agent
template write (`TemplateCreate`/`TemplateEdit`) or `TemplateDelete` approval card
(crates/gui/farhelm-gui/src/approvals/model.rs:832-839) and the Templates list summary
(crates/gui/farhelm-gui/src/templates/summary.rs:100) both invoke `permission_label(fields.agent, *permissions)` and
misreport the stored field: e.g. `Claude + Approve` displays `"Guarded · Claude auto mode"`, `Cursor + Guarded` displays
`"YOLO"` (previously `"Agent's own setting"` before 7b226839/6d32efc9 changed Cursor's omitted_permission to YOLO), and
`Pi + Custom` displays `"YOLO"`. SPEC.md (Agent-spawned sessions, lines 4592-4593) specifies "Template cards show the
permission field when present, resolving a reset to the agent's default when the template names one" — resolving only a
reset (`None`), not rewriting an explicit stored permission. Compare approvals/model_tests.rs:848-863, which explicitly
requires shell template cards to show incompatible stored permissions accurately.

How to verify: Call `templates::summary::permission_label(Some(LaunchHarness::Claude), Some(LaunchPermission::Approve))`
(returns `"Guarded · Claude auto mode"` instead of `"Approve"`) or
`permission_label(Some(LaunchHarness::Cursor), Some(LaunchPermission::Guarded))` (returns `"YOLO"` instead of
`"Guarded"`).

Fix: In crates/gui/farhelm-gui/src/templates/summary.rs:192-198, resolve only `None` (an explicit reset) via
`harness.omitted_permission()`, and display any explicit `Some(chosen)` as itself (using `guarded_mode_name(harness)`
only when `chosen == LaunchPermission::Guarded` and supported, falling back to `permission_display(Some(chosen))`).

Reviewer metadata (g20_yolo_unset_gui_launcher/l4): suggested bucket: other — misstates incompatible stored permissions
on template summaries and approval cards, but cannot disguise a YOLO or Custom permission as Guarded: every non-shell
harness offers `Yolo` (so `Yolo` never falls back), every non-shell harness that lacks `Custom` (`Goose`, `Pi`,
`OpenCode`, `Custom`) has `omitted_permission() == Some(Yolo)` (so `Custom` falls back to `YOLO` while
`permission_flags` still prints on its own fact line), and `apply_template`/`launches::compile` refuse unsupported
`(agent, permission)` combinations when launching. Thus no security bypass or data-loss route is established.;
confidence: confirmed; possible cover: none.

Review evidence is retained under run `746bd2ca-c078-44c7-90d6-305e2a679dca` as `g20_yolo_unset_gui_launcher/l4`.

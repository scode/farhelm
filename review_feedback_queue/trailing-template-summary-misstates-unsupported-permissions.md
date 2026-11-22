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

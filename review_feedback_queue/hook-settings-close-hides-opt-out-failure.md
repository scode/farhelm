# Closing Settings can hide a refusal to turn off automatic hook setup

Reviewed commit: bb31959c2f22c96bbbb72da2e60eee4a0b288a6b

## TLDR

Closing Settings while an opt-out saves can hide the refusal and leave automatic hook setup active.

## Details

**high.** Source inspection.

bb31959c2f22c96bbbb72da2e60eee4a0b288a6b crates/gui/farhelm-gui/src/settings.rs:252-263,864-868,1175-1197;
crates/gui/farhelm-gui/src/settings/editing.rs:236-267,552-562;
crates/gui/farhelm-gui/src/settings/configured.rs:103-110,119-136;
crates/gui/farhelm-gui/src/settings/hook_settings.rs:46-60; crates/gui/farhelm-gui/src/drafts.rs:141-166;
crates/gui/farhelm-gui/src/operations.rs:912-923; crates/farhelm-helm/src/agent_hook_setup.rs:52-73;
crates/farhelm-helm/src/sessions.rs:2179-2192

The user can see automatic hook setup switch off, close Settings, and receive no indication that the helm refused the
save. The close check inventories explicit form editors and returns Close when that list is empty, even during a hook
switch save. The operation outlives the dialog but its callback holds a weak reference to the Settings draft. A refusal
remains on the shared operation key while the hook row reads its own draft or GET error, and a fresh dialog creates a
clean draft. No other hook settings surface consumes that failure. The persisted on setting can still be used for later
starts; existing hooks also remain as specified irrespective of opt-out. SPEC.md at
1a53d9d35889d7dde96e31320cc1681dd013737d:4763-4793,5567-5570 requires surfacing failures and names individual Settings
controls whose close may drop alerts. Hook setup is outside those named exceptions. Keep a pending hook consent write
visible through close or route its late result to a surviving, agent-specific notification. Source establishes the close
and display paths. Incidence of a failed disable after closure, and any later file write, are unmeasured. The reviewer
suggested high for silently failing an all-hosts opt-out. The underlying setting retains the former consent, so this
route alone does not establish a new unauthorized write. The generic draft lifetime also affects other controls, but
this finding concerns the new hook setting.

Review evidence is retained under run `e551ad05-1866-44a9-b97b-9e47d1d67375` as `hook-settings/HS-8`.

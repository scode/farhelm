# Three layout preferences can silently revert after a failed save

Reviewed commit: 1a5b4a1ff6b1deb4551cfaf064176cbb6e174b9d

## TLDR

The sidebar width, terminal text size and host rail expansion can briefly change and then revert without telling you the
helm could not save them.

## Details

Bucket: **other**. GUI migration final whole-tree review, R8 Sol layout saves.

Source references: `crates/gui/farhelm-gui/src/sidebar.rs:273`, `crates/gui/farhelm-gui/src/terminal/text_size.rs:156`,
`crates/gui/farhelm-gui/src/session_list/host_rail/expansion.rs:128`,
`crates/gui/farhelm-gui/src/session_list/queued_preference.rs:146`, `crates/farhelm-helm/src/preferences.rs:315`.

Resize the sidebar, change terminal text size, or expand or collapse the host rail while the connection is healthy but
the helm cannot commit the preference. The affected view logs the refusal and drops its optimistic display hold. The
host rail shares its log-only refusal handler with Compact. The old value remains saved.

Sol rated this source-confirmed and noncritical. There is no separate verification verdict. SPEC.md “Errors and
diagnostics” lists Compact among the best-effort exceptions but does not list these three. “Healthy local filesystems”
accepts the storage failure used as an example. Whether the GUI is allowed to hide the refusal on an otherwise working
connection is a separate question.

Report the refusal in the affected surface while restoring the previous preference. Cover each of the three controls
with a fake rejected write.
